#!/usr/bin/env python3
"""Per-worker state paths, atomic JSON writes, and shared issue/PR claims.

Several workers (worker, worker-2, ...) may run in parallel, each in its own
worktree. Each worker has its own directory with `state.json` and `tick.lock`.
`claims.json` is shared: a claim on `issue:N` or `pr:N` names the worker that
owns it, so two workers never take the same issue or PR.

State lives outside the user's checkout:
`<repo>/../<repo-name>.worktrees/.worker-loop/` (override with $LOOP_STATE_DIR).

CLI:
  worker_state.py claim   --worker W --ref issue:N   exit 0 claimed (or already ours), 1 held by another worker
  worker_state.py release --worker W --ref pr:N      release our own claim
  worker_state.py list                               print claims.json
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path

DEFAULT_WORKER = "worker"
WORKER_NAME = re.compile(r"^[a-z][a-z0-9-]{0,31}$")
REF_FORMAT = re.compile(r"^(issue|pr):\d+$")
# A crashed writer must not block claims forever; claim updates take milliseconds.
CLAIMS_LOCK_STALE_SECONDS = 60


def worker_name(explicit: str | None = None) -> str:
    name = explicit or os.environ.get("WORKER_ID") or DEFAULT_WORKER
    if not WORKER_NAME.match(name):
        raise ValueError(f"invalid worker name: {name!r}")
    return name


def state_root() -> Path:
    env = os.environ.get("LOOP_STATE_DIR")
    if env:
        return Path(env)
    out = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"], capture_output=True, text=True, check=True
    ).stdout.strip()
    common = Path(out).resolve()
    repo_root = common.parent if common.name == ".git" else common
    return repo_root.parent / f"{repo_root.name}.worktrees" / ".worker-loop"


def worker_dir(worker: str) -> Path:
    return state_root() / worker


def claims_path() -> Path:
    return state_root() / "claims.json"


def atomic_write_json(path: Path, data: dict) -> None:
    """Write JSON through a temp file and rename, so readers never see a partial file."""
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=path.parent, prefix=f".{path.name}.", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        os.replace(tmp, path)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise


def read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


@contextmanager
def exclusive(lock_path: Path, timeout: float = 10.0):
    """Cross-process mutex via O_EXCL lock file; serializes read-modify-write of a shared file."""
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + timeout
    while True:
        try:
            fd = os.open(lock_path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            os.close(fd)
            break
        except FileExistsError:
            try:
                if time.time() - lock_path.stat().st_mtime > CLAIMS_LOCK_STALE_SECONDS:
                    lock_path.unlink(missing_ok=True)
                    continue
            except FileNotFoundError:
                continue
            if time.monotonic() > deadline:
                raise TimeoutError(f"could not acquire {lock_path}")
            time.sleep(0.05)
    try:
        yield
    finally:
        lock_path.unlink(missing_ok=True)


def load_claims(path: Path | None = None) -> dict:
    return read_json(path or claims_path())


def claim(worker: str, ref: str, path: Path | None = None) -> str | None:
    """Claim `ref` for `worker`. Returns None on success, else the worker holding it."""
    if not REF_FORMAT.match(ref):
        raise ValueError(f"invalid ref: {ref!r} (expected issue:N or pr:N)")
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        holder = (claims.get(ref) or {}).get("worker")
        if holder and holder != worker:
            return holder
        claims[ref] = {"worker": worker, "at": int(time.time())}
        atomic_write_json(path, claims)
    return None


def release(worker: str, ref: str, path: Path | None = None) -> bool:
    """Release `ref` if `worker` holds it. Returns True when a claim was removed."""
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        if (claims.get(ref) or {}).get("worker") != worker:
            return False
        del claims[ref]
        atomic_write_json(path, claims)
    return True


def claimed_by_other(claims: dict, ref: str, worker: str) -> str | None:
    holder = (claims.get(ref) or {}).get("worker")
    return holder if holder and holder != worker else None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("command", choices=["claim", "release", "list"])
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID or 'worker')")
    parser.add_argument("--ref", default=None, help="issue:N or pr:N")
    args = parser.parse_args()

    if args.command == "list":
        print(json.dumps(load_claims(), ensure_ascii=False, indent=2))
        return
    if not args.ref:
        sys.exit("--ref is required")
    worker = worker_name(args.worker)
    if args.command == "claim":
        holder = claim(worker, args.ref)
        if holder:
            print(f"HELD by {holder}")
            sys.exit(1)
        print(f"CLAIMED {args.ref} for {worker}")
    else:
        print("RELEASED" if release(worker, args.ref) else "NOT HELD")


if __name__ == "__main__":
    main()
