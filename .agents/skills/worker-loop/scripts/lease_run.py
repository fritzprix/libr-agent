#!/usr/bin/env python3
"""Run a command while keeping a tick's lease renewed.

  lease_run.py --worker W --token T -- CMD...   the tick owns the lease
  lease_run.py --acquire [--worker W] -- CMD... wrapper: own the lease for CMD's whole run

The lease is checked and renewed before CMD starts, so this is also the fence for
destructive git commands and pushes: a run that lost its lease never executes them.
While CMD runs, the lease is renewed every --interval seconds; if it is lost, CMD
is terminated and the exit code is 4. Otherwise the exit code is CMD's.

With --acquire, the command receives WORKER_ID and WORKER_LOOP_INHERITED_TOKEN. Its
`tick_gate.py --lock` then reuses this lease instead of reporting a duplicate tick,
and its `--unlock` leaves the lease for this wrapper to release on every exit path.
"""
from __future__ import annotations

import argparse
import os
import signal
import subprocess
import sys
import threading
from pathlib import Path

import lease
import worker_slots
import worker_state

DEFAULT_INTERVAL_SECONDS = 300.0
TERMINATE_GRACE_SECONDS = 10.0
EXIT_BUSY = 2
EXIT_LEASE_LOST = worker_state.EXIT_LEASE_LOST


def _terminate(proc: subprocess.Popen) -> None:
    """End CMD and everything it started (cargo, rustc, linkers)."""
    if proc.poll() is not None:
        return
    if sys.platform == "win32":
        subprocess.run(["taskkill", "/T", "/F", "/PID", str(proc.pid)], capture_output=True)
        return
    try:
        os.killpg(proc.pid, signal.SIGTERM)
        proc.wait(timeout=TERMINATE_GRACE_SECONDS)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass


def run_with_lease(worker: str, token: str, cmd: list[str], interval: float, env: dict, state_file: Path | None) -> int:
    if not lease.renew(worker, token, state_file):
        print(f"LEASE LOST: {worker} is not held by this token; command not run", file=sys.stderr)
        return EXIT_LEASE_LOST

    new_group = (
        {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
        if sys.platform == "win32"
        else {"start_new_session": True}
    )
    proc = subprocess.Popen(cmd, env=env, **new_group)
    done = threading.Event()
    lost = threading.Event()

    def keep_renewing() -> None:
        while not done.wait(interval):
            try:
                renewed = lease.renew(worker, token, state_file)
            except (TimeoutError, OSError, worker_state.StateFileCorrupt) as e:
                # Says nothing about ownership: try again at the next interval.
                print(f"lease_run: renewal failed, will retry: {e}", file=sys.stderr)
                continue
            if not renewed:
                lost.set()
                print(f"LEASE LOST: {worker} was taken over; stopping the command", file=sys.stderr)
                _terminate(proc)
                return

    def forward(signum, _frame):
        _terminate(proc)
        raise SystemExit(128 + signum)

    previous = {s: signal.signal(s, forward) for s in (signal.SIGINT, signal.SIGTERM)}
    renewer = threading.Thread(target=keep_renewing, daemon=True)
    renewer.start()
    try:
        code = proc.wait()
    finally:
        done.set()
        renewer.join(timeout=TERMINATE_GRACE_SECONDS + 1)
        for signum, handler in previous.items():
            signal.signal(signum, handler)
    return EXIT_LEASE_LOST if lost.is_set() else code


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--worker", default=None)
    parser.add_argument("--token", default=None)
    parser.add_argument("--acquire", action="store_true", help="Own a new lease for the command's whole run")
    parser.add_argument("--session", default=None)
    parser.add_argument("--pid", type=int, default=None)
    parser.add_argument("--interval", type=float, default=DEFAULT_INTERVAL_SECONDS)
    parser.add_argument("--state-file", type=Path, default=None)
    parser.add_argument("cmd", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    cmd = args.cmd[1:] if args.cmd[:1] == ["--"] else args.cmd
    if not cmd:
        parser.error("no command given")

    env = dict(os.environ)
    if not args.acquire:
        token = args.token or env.get("WORKER_LOOP_TOKEN") or env.get(lease.INHERITED_TOKEN_ENV)
        worker = args.worker or env.get("WORKER_ID") or (token and worker_slots.find_worker_by_token(token))
        if not worker or not token:
            parser.error("--worker and --token are required unless --acquire is given")
        return run_with_lease(worker, token, cmd, args.interval, env, args.state_file)

    owner = worker_state.resolve_owner(args.pid, args.session)
    try:
        held = lease.acquire(owner, args.worker or env.get("WORKER_ID"), args.state_file)
    except worker_slots.SlotBusy as e:
        print(f"BUSY reason={e.reason} ({e})")
        return EXIT_BUSY
    env["WORKER_ID"] = held.worker
    env[lease.INHERITED_TOKEN_ENV] = held.token
    try:
        return run_with_lease(held.worker, held.token, cmd, args.interval, env, args.state_file)
    finally:
        lease.release(held.worker, held.token, args.state_file)


if __name__ == "__main__":
    sys.exit(main())
