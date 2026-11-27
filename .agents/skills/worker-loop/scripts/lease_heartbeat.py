#!/usr/bin/env python3
"""Renew this tick's lease from its own process, so a quiet tick keeps it (#3031).

A tick that only runs raw commands never calls `tick_gate.py --renew`, and its
lease dies `LEASE_TTL_SECONDS` (1800s) after `--lock`. `lease.acquire` starts
this heartbeat, so the contract holds in code, not only in the skill text.

It stops on its own when

  * the watched process is gone — the process that lives as long as the agent
    session (`Owner.pid`); under LibrAgent that is the app process, because a
    command's own parent is the app, not a per-command shell (measured: the
    shell's PPID is the `libragent` process),
  * renewal fails — the lease now belongs to another run,
  * it has renewed for `MAX_LIFETIME_SECONDS` (2h): app-scoped owners have no
    per-tick process to watch, so their heartbeat would otherwise renew for as
    long as the app lives — through a crash, which is what the orphaned-slot
    takeover and the TTL are for. The ceiling is what finally stops renewing
    for such a tick and lets the TTL reclaim the slot.

`lease.release` (every `--unlock` path) stops it explicitly for that lease's
token, so a run that lost the slot cannot stop the new owner's heartbeat.
`worker_slots.set_heartbeat_pid` records its PID together with the process start
time, and the stop path signals only while the PID still has that start: a
reused PID must never be killed.
"""
from __future__ import annotations

import argparse
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import worker_slots
import worker_state

INTERVAL_ENV = "WORKER_LOOP_HEARTBEAT_SECONDS"
MAX_LIFETIME_ENV = "WORKER_LOOP_HEARTBEAT_MAX_SECONDS"
DEFAULT_INTERVAL_SECONDS = 300.0  # six renewals per 1800s TTL
DEFAULT_MAX_LIFETIME_SECONDS = 2 * 3600.0
_PID_KEY = "heartbeatPid"
_START_KEY = "heartbeatStart"

# Windows has neither zombies nor `WNOHANG`, and no `SIGKILL`; there `os.kill`
# terminates outright. Both are looked up once so the stop path never raises on
# a platform that lacks them (#3031 G1.2).
_WNOHANG = getattr(os, "WNOHANG", None)
_SIGKILL = getattr(signal, "SIGKILL", None)

# Heartbeats this process started and has not reaped yet, by PID. Kept so the
# `Popen` handle survives until `_reap` collects the exit status.
_spawned: dict[int, subprocess.Popen] = {}


def _env_seconds(name: str, default: float) -> float:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return default
    try:
        value = float(raw)
    except ValueError:
        raise ValueError(f"{name} must be a number of seconds, got {raw!r}") from None
    if value <= 0:
        raise ValueError(f"{name} must be positive, got {value}")
    return value


def interval_seconds() -> float:
    return _env_seconds(INTERVAL_ENV, DEFAULT_INTERVAL_SECONDS)


def max_lifetime_seconds() -> float:
    return _env_seconds(MAX_LIFETIME_ENV, DEFAULT_MAX_LIFETIME_SECONDS)


def heartbeat(worker: str, token: str | None = None) -> tuple[int | None, str | None]:
    """The recorded heartbeat PID and its start time; `(None, None)` when none.

    `token` limits the answer to the lease that owns the slot, so a run that has
    lost its lease cannot stop the heartbeat of the run that took it over
    (#3031 G1.4).
    """
    record = worker_slots.load_slots().get(worker) or {}
    if token is not None and record.get("token") != token:
        return (None, None)
    pid = record.get(_PID_KEY)
    start = record.get(_START_KEY)
    return (
        int(pid) if isinstance(pid, int) and pid > 0 else None,
        start if isinstance(start, str) else None,
    )


def heartbeat_pid(worker: str) -> int | None:
    """The recorded heartbeat PID, live or not; None when the slot has none."""
    return heartbeat(worker)[0]


def running(pid: int) -> bool:
    """Alive and not a zombie this process still has to reap.

    In production the heartbeat is re-parented to init, which reaps it. A
    process that started one and lives on — a test, or a tick that ended —
    keeps the exit status until it reaps, and `kill(pid, 0)` answers "alive"
    for that zombie. Windows has no zombies, so `pid_alive` is the whole answer
    there.
    """
    if not worker_state.pid_alive(pid):
        return False
    if _WNOHANG is None:
        return True
    try:
        reaped, _ = os.waitpid(pid, _WNOHANG)
    except (ChildProcessError, OSError):
        return True  # not our child: pid_alive is all we can know
    if reaped == 0:
        return True
    # We just reaped it: let the handle learn that, or `Popen.__del__` warns.
    proc = _spawned.pop(pid, None)
    if proc is not None:
        proc.poll()
    return False


def _is_the_heartbeat(pid: int, start: str | None) -> bool:
    """Does `pid` still name the process this lease started?

    A dead process's PID is reused, so a recorded PID alone can point at an
    unrelated process, and signalling that one is machine-unsafe (#3031 G1.3).
    The recorded start time is the tie-breaker; where it could not be read there
    is nothing better than the PID.
    """
    if not running(pid):
        return False
    return start is None or worker_state.pid_start(pid) == start


def start(
    worker: str,
    token: str,
    watch_pid: int | None = None,
    state_file: Path | None = None,
) -> int | None:
    """Start the heartbeat for this lease, or reuse the one already running.

    Returns its PID, or None when it could not be recorded: the caller keeps a
    usable lease either way (`--renew` and `lease_run.py` still extend it).
    """
    existing = heartbeat_pid(worker)
    if existing and running(existing):
        return existing
    command = [
        sys.executable,
        str(Path(__file__).resolve()),
        "--worker",
        worker,
        "--token",
        token,
        "--interval",
        str(interval_seconds()),
        "--max-lifetime",
        str(max_lifetime_seconds()),
    ]
    if watch_pid:
        command += ["--watch-pid", str(watch_pid)]
    if state_file:
        command += ["--state-file", str(state_file)]
    proc = subprocess.Popen(
        command,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        # Own session: the launching shell (and a Ctrl-C in it) must not take
        # the heartbeat down, and it must outlive the `--lock` process.
        start_new_session=True,
        close_fds=True,
    )
    # Hold the handle until the heartbeat is reaped: a discarded `Popen` whose
    # child still runs prints a ResourceWarning from `__del__` on every `--lock`.
    _spawned[proc.pid] = proc
    if not worker_slots.set_heartbeat_pid(worker, token, proc.pid):
        _terminate(proc.pid, worker_state.pid_start(proc.pid))
        return None
    return proc.pid


def stop(worker: str, token: str | None = None, timeout: float = 5.0) -> bool:
    """Stop the recorded heartbeat, if any. Returns True when one was running.

    `token` stops only the heartbeat of the lease that owns the slot: a run that
    lost its lease must not take down the new owner's heartbeat (#3031 G1.4).
    """
    pid, start = heartbeat(worker, token)
    if not pid:
        return False
    return _terminate(pid, start, timeout)


def _terminate(pid: int, start: str | None = None, timeout: float = 5.0) -> bool:
    stopped = _kill(pid, start, timeout)
    # Only collect a status we caused. A refused signal leaves a live process,
    # and reaping it would either block or kill a PID the stop path just
    # declined to touch (#3031 G1.3).
    if stopped:
        _reap(pid)
    return stopped


def _reap(pid: int) -> None:
    """Collect the exit status of a heartbeat this process started.

    Only `_spawned` holds a handle; a heartbeat started by another process is
    not ours to wait on. `running` already reaps a child that died on its own,
    so this sees a process this process stopped.
    """
    proc = _spawned.pop(pid, None)
    if proc is None:
        return
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        _spawned[pid] = proc


def _kill(pid: int, start: str | None = None, timeout: float = 5.0) -> bool:
    """Signal the heartbeat, but only while `pid` is still that process.

    The start time is re-checked before each signal: between the polite SIGTERM
    and the forced one the process may have exited and its PID been reused
    (#3031 G1.3). Without `SIGKILL` (Windows) the SIGTERM above already ends the
    process, so there is no second stage.
    """
    if not _is_the_heartbeat(pid, start):
        return False
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return False
    deadline = time.time() + timeout
    while time.time() < deadline:
        if not _is_the_heartbeat(pid, start):
            return True
        time.sleep(0.05)
    if _SIGKILL is not None and _is_the_heartbeat(pid, start):
        try:
            os.kill(pid, _SIGKILL)
        except OSError:
            pass
    for _ in range(int(timeout / 0.05) or 1):
        if not _is_the_heartbeat(pid, start):
            break
        time.sleep(0.05)
    return True


def run(
    worker: str,
    token: str,
    watch_pid: int | None,
    interval: float,
    max_lifetime: float,
    state_file: Path | None = None,
) -> int:
    """The heartbeat loop itself: renew until the lease or the watch ends."""
    import lease  # local import: lease imports this module at load time

    started = time.time()
    try:
        while True:
            # A lease that is already gone — a released slot, a deleted state
            # directory — ends the heartbeat at once. Without this the wait
            # below would keep a heartbeat alive for one interval after its
            # lease stopped existing.
            if not lease.is_held_by(worker, token, state_file):
                return 0
            if time.time() - started >= max_lifetime:
                return 0
            # `lease.acquire` has just written a fresh lease, so the first
            # renewal waits one interval. Renewing the moment this starts made
            # the heartbeat race every test, takeover and operator that ages a
            # lease to check whether it is still live (#3031 G1.1).
            time.sleep(interval)
            if watch_pid and not worker_state.pid_alive(watch_pid):
                return 0
            if not lease.renew(worker, token, state_file):
                return 0
    finally:
        worker_slots.set_heartbeat_pid(worker, token, None)


def main() -> None:
    parser = argparse.ArgumentParser(description="Renew one worker lease until it ends.")
    parser.add_argument("--worker", required=True)
    parser.add_argument("--token", required=True)
    parser.add_argument("--watch-pid", type=int, default=None)
    parser.add_argument("--interval", type=float, default=None)
    parser.add_argument("--max-lifetime", type=float, default=None)
    parser.add_argument("--state-file", type=Path, default=None)
    args = parser.parse_args()
    sys.exit(
        run(
            worker_state.worker_name(args.worker),
            args.token,
            args.watch_pid,
            args.interval if args.interval is not None else interval_seconds(),
            args.max_lifetime if args.max_lifetime is not None else max_lifetime_seconds(),
            args.state_file,
        )
    )


if __name__ == "__main__":
    main()
