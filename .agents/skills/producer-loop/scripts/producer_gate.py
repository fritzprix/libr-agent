#!/usr/bin/env python3
"""Due-task gate for producer-loop.

Picks the producer task that is most overdue and manages tick.lock. Lock, state
and path helpers come from worker-loop/scripts/tick_gate.py, so TTL and PID
semantics stay identical.

Usage:
  producer_gate.py [--lock] [--pid N]   -> RUN <task> | NOOP | BUSY
  producer_gate.py --task T [--lock]    -> RUN T (run one task now, due or not)
  producer_gate.py --done T [--unlock]  -> record a finished run of T
  producer_gate.py --unlock             -> release tick.lock
  producer_gate.py --status             -> print periods and last runs

Exit codes: 0 RUN or done, 1 NOOP, 2 BUSY, 3 ERROR.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import time
from pathlib import Path

EXIT_RUN = 0
EXIT_NOOP = 1
EXIT_BUSY = 2
EXIT_ERROR = 3

TASKS = ("security", "e2e", "uiux", "regression", "usage-logs", "doc-drift", "cleanup")
DEFAULT_PERIOD_SECONDS = 24 * 60 * 60  # one day; override per task in state.json "periods"

_TICK_GATE_PATH = Path(__file__).resolve().parents[2] / "worker-loop" / "scripts" / "tick_gate.py"


def _load_tick_gate():
    spec = importlib.util.spec_from_file_location("tick_gate", _TICK_GATE_PATH)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


tick_gate = _load_tick_gate()


def default_state_path() -> Path:
    """Same resolution as tick_gate.default_state_path, for the producer-loop directory."""
    return tick_gate.default_state_path().parent.parent / "producer-loop" / "state.json"


def period_seconds(state: dict, task: str) -> int:
    value = (state.get("periods") or {}).get(task)
    if isinstance(value, (int, float)) and not isinstance(value, bool) and value > 0:
        return int(value)
    return DEFAULT_PERIOD_SECONDS


def select_due_task(state: dict, now: float) -> str | None:
    """Return the most overdue task, or None when no task is due. Ties keep TASKS order."""
    last_runs = state.get("lastRun") or {}
    best: tuple[float, str] | None = None
    for task in TASKS:
        overdue = now - float(last_runs.get(task, 0)) - period_seconds(state, task)
        if overdue >= 0 and (best is None or overdue > best[0]):
            best = (overdue, task)
    return best[1] if best else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--state-file", type=Path, default=None)
    parser.add_argument("--task", choices=TASKS, help="Run this task now, due or not")
    parser.add_argument("--done", choices=TASKS, help="Record a finished run of this task")
    parser.add_argument("--lock", action="store_true", help="Acquire tick.lock on RUN")
    parser.add_argument("--pid", type=int, default=None)
    parser.add_argument("--unlock", action="store_true", help="Release tick.lock")
    parser.add_argument("--status", action="store_true")
    args = parser.parse_args(argv)

    state_path = args.state_file or default_state_path()
    lock_path = state_path.parent / "tick.lock"
    state = tick_gate.load_state(state_path)

    try:
        if args.status:
            last_runs = state.get("lastRun") or {}
            for task in TASKS:
                print(f"{task}: period={period_seconds(state, task)}s lastRun={last_runs.get(task, 'never')}")
            return EXIT_RUN

        if args.done:
            state.setdefault("lastRun", {})[args.done] = int(time.time())
            tick_gate.save_state(state_path, state)
            print(f"DONE {args.done}")
            if args.unlock:
                tick_gate.release_lock(lock_path)
                print("UNLOCKED")
            return EXIT_RUN

        if args.unlock:
            tick_gate.release_lock(lock_path)
            print("UNLOCKED")
            return EXIT_RUN

        if tick_gate.check_lock(lock_path):
            print("BUSY (tick in progress)")
            return EXIT_BUSY

        task = args.task or select_due_task(state, time.time())
        if task is None:
            print("NOOP")
            return EXIT_NOOP
        if args.lock:
            tick_gate.acquire_lock(lock_path, pid=args.pid)
        print(f"RUN {task}")
        return EXIT_RUN
    except Exception as e:  # noqa: BLE001 - report any gate failure as ERROR, never as NOOP
        sys.stderr.write(f"producer_gate error: {e}\n")
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
