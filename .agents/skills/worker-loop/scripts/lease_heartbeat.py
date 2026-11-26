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

`lease.release` (every `--unlock` path) stops it explicitly, and
`worker_slots.set_heartbeat_pid` records its PID on the slot record so any
release path can find it.
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


def heartbeat_pid(worker: str) -> int | None:
    """The recorded heartbeat PID, live or not; None when the slot has none."""
    record = worker_slots.load_slots().get(worker) or {}
    pid = record.get(_PID_KEY)
    return int(pid) if isinstance(pid, int) and pid > 0 else None


def running(pid: int) -> bool:
    """Alive and not a zombie this process still has to reap.

    In production the heartbeat is re-parented to init, which reaps it. A
    process that started one and lives on — a test, or a tick that ended —
    keeps the exit status until it reaps, and `kill(pid, 0)` answers "alive"
    for that zombie.
    """
    if not worker_state.pid_alive(pid):
        return False
    try:
        reaped, _ = os.waitpid(pid, os.WNOHANG)
    except (ChildProcessError, OSError):
        return True  # not our child: pid_alive is all we can know
    if reaped == 0:
        return True
    # We just reaped it: let the handle learn that, or `Popen.__del__` warns.
    proc = _spawned.pop(pid, None)
    if proc is not None:
        proc.poll()
    return False


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
        _terminate(proc.pid)
        return None
    return proc.pid


def stop(worker: str, timeout: float = 5.0) -> bool:
    """Stop the recorded heartbeat, if any. Returns True when one was running."""
    pid = heartbeat_pid(worker)
    if not pid:
        return False
    return _terminate(pid, timeout)


def _terminate(pid: int, timeout: float = 5.0) -> bool:
    try:
        return _kill(pid, timeout)
    finally:
        _reap(pid)


def _reap(pid: int) -> None:
    """Collect the exit status of a heartbeat this process started.

    Only `_spawned` holds a handle; a heartbeat started by another process is
    not ours to wait on.
    """
    proc = _spawned.pop(pid, None)
    if proc is None:
        return
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()


def _kill(pid: int, timeout: float = 5.0) -> bool:
    if not running(pid):
        return False
    try:
        os.kill(pid, signal.SIGTERM)
    except OSError:
        return False
    deadline = time.time() + timeout
    while time.time() < deadline:
        if not running(pid):
            return True
        time.sleep(0.05)
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError:
        pass
    for _ in range(int(timeout / 0.05) or 1):
        if not running(pid):
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
            if watch_pid and not worker_state.pid_alive(watch_pid):
                return 0
            if time.time() - started >= max_lifetime:
                return 0
            if not lease.renew(worker, token, state_file):
                return 0
            time.sleep(interval)
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
