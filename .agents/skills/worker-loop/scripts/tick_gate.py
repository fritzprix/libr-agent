#!/usr/bin/env python3
"""Preflight gate for worker-loop.

Computes a fingerprint of open PRs (heads, mergeable state, updatedAt) and candidate
issues (updatedAt). If state.json has no 'current' and the fingerprint has not changed
since the last run, outputs 'NOOP' and exits with code 1 so external schedulers
(e.g., `python3 tick_gate.py && claude -p "/worker-loop"`) do not invoke the LLM.

Exits:
  0 - RUN (actionable work or state changed)
  1 - NOOP (state unchanged and idle)
  2 - BUSY (another tick is currently running with valid lock)
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path


def default_state_path() -> Path:
    env = os.environ.get("LOOP_STATE_DIR")
    if env:
        p = Path(env)
        return (
            p
            if p.name == "state.json"
            else (p if p.name == "worker-loop" else p / "worker-loop") / "state.json"
        )
    try:
        out = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            capture_output=True, text=True, check=True
        ).stdout.strip()
        common = Path(out).resolve()
        repo_root = common.parent if common.name == ".git" else common
        return repo_root / ".libragent" / "work" / "worker-loop" / "state.json"
    except Exception:
        return Path(".libragent/work/worker-loop/state.json")


def load_state(path: Path) -> dict:
    if path.is_file():
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            return {}
    return {}


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2), encoding="utf-8")


def gh(*args: str) -> str:
    result = subprocess.run(["gh", *args], capture_output=True, text=True, encoding="utf-8")
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed: {result.stderr.strip()}")
    return result.stdout


def check_lock(lock_file: Path, ttl_seconds: int = 900) -> bool:
    """Return True if a valid active lock exists."""
    if not lock_file.is_file():
        return False
    try:
        data = json.loads(lock_file.read_text(encoding="utf-8"))
        locked_at = data.get("time", 0)
        pid = data.get("pid")
        if time.time() - locked_at > ttl_seconds:
            # Stale lock
            return False
        if pid and sys.platform != "win32":
            try:
                os.kill(pid, 0)
            except OSError:
                # Process no longer exists
                return False
        return True
    except Exception:
        return False


def acquire_lock(lock_file: Path) -> None:
    lock_file.parent.mkdir(parents=True, exist_ok=True)
    payload = {"pid": os.getpid(), "time": int(time.time())}
    lock_file.write_text(json.dumps(payload), encoding="utf-8")


def release_lock(lock_file: Path) -> None:
    if lock_file.is_file():
        try:
            lock_file.unlink()
        except OSError:
            pass


def compute_fingerprint() -> str:
    """Compute sha256 hash of open PR states and recent issue update timestamps."""
    hasher = hashlib.sha256()

    try:
        prs_raw = gh("pr", "list", "--author", "@me", "--state", "open", "--json",
                     "number,headRefOid,mergeable,updatedAt,isDraft")
        prs = json.loads(prs_raw)
        prs_norm = sorted(
            (p["number"], p.get("headRefOid"), p.get("mergeable"), p.get("updatedAt"), p.get("isDraft"))
            for p in prs
        )
        hasher.update(json.dumps(prs_norm).encode("utf-8"))
    except Exception as e:
        hasher.update(f"prs_err:{e}".encode("utf-8"))

    try:
        issues_raw = gh("issue", "list", "--state", "open", "--limit", "30", "--json",
                        "number,updatedAt")
        issues = json.loads(issues_raw)
        issues_norm = sorted((i["number"], i.get("updatedAt")) for i in issues)
        hasher.update(json.dumps(issues_norm).encode("utf-8"))
    except Exception as e:
        hasher.update(f"issues_err:{e}".encode("utf-8"))

    return hasher.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-file", type=Path, default=None, help="Path to state.json")
    parser.add_argument("--update", action="store_true", help="Update lastFingerprint in state.json")
    parser.add_argument("--lock", action="store_true", help="Acquire tick.lock before exiting 0")
    parser.add_argument("--unlock", action="store_true", help="Release tick.lock")
    args = parser.parse_args()

    state_path = args.state_file or default_state_path()
    lock_path = state_path.parent / "tick.lock"

    if args.unlock:
        release_lock(lock_path)
        print("UNLOCKED")
        sys.exit(0)

    if check_lock(lock_path):
        print("BUSY (tick in progress)")
        sys.exit(2)

    state = load_state(state_path)

    # If currently implementing an issue or branch, always run
    if state.get("current"):
        if args.lock:
            acquire_lock(lock_path)
        print("RUN (current work in progress)")
        sys.exit(0)

    fp = compute_fingerprint()
    last_fp = state.get("lastFingerprint")

    if args.update:
        state["lastFingerprint"] = fp
        save_state(state_path, state)
        print(f"UPDATED: {fp[:12]}")
        sys.exit(0)

    if last_fp and last_fp == fp:
        print("NOOP (no changes)")
        sys.exit(1)

    # State changed
    if args.lock:
        acquire_lock(lock_path)
    state["lastFingerprint"] = fp
    save_state(state_path, state)
    print(f"RUN (changes detected: {fp[:12]})")
    sys.exit(0)


if __name__ == "__main__":
    main()
