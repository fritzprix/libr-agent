#!/usr/bin/env python3
"""Per-worker state paths, atomic JSON writes, shared claims, and session check-ins.

Several workers (worker, worker-2, ...) may run in parallel, each in its own
worktree. Each worker has its own directory with `state.json` and `tick.lock`.
`claims.json` is shared: a claim on `issue:N` or `pr:N` names the worker that
owns it, so two workers never take the same issue or PR.

When no `--worker` / $WORKER_ID is set, a session checks in under `checkins.json`
and receives the first free slot (`worker`, `worker-2`, ...). Check-ins are
refreshed during long ticks via `heartbeat`; stale or dead holders can be replaced.

State lives outside the user's checkout:
`<repo>/../<repo-name>.worktrees/.worker-loop/` (override with $LOOP_STATE_DIR).

CLI:
  worker_state.py claim   --worker W --ref issue:N   exit 0 claimed (or already ours), 1 held by another worker
  worker_state.py release --worker W --ref pr:N      release our own claim
  worker_state.py list                               print claims.json
  worker_state.py checkin acquire [--worker W]       claim a slot; print WORKER line
  worker_state.py checkin release [--worker W]       release our slot (defaults to this pid)
  worker_state.py checkin heartbeat --worker W       refresh heartbeat for long commands
"""
from __future__ import annotations

import argparse
import json
import os
import re
import socket
import subprocess
import sys
import tempfile
import time
from contextlib import contextmanager
from pathlib import Path

DEFAULT_WORKER = "worker"
WORKER_NAME = re.compile(r"^[a-z][a-z0-9-]{0,31}$")
REF_FORMAT = re.compile(r"^(issue|pr):\d+$")
SLOT_NAME = re.compile(r"^worker(?:-\d+)?$")
# A crashed writer must not block claims forever; claim updates take milliseconds.
CLAIMS_LOCK_STALE_SECONDS = 60
# No heartbeat refresh for this long → slot may be taken over (holder pid may still be alive).
CHECKIN_STALE_SECONDS = 600
MAX_AUTO_SLOTS = 32


class CheckinError(RuntimeError):
    """Could not acquire or refresh a worker check-in slot."""


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


def checkins_path() -> Path:
    return state_root() / "checkins.json"


def _host() -> str:
    try:
        return socket.gethostname()
    except OSError:
        return "unknown"


def _pid_alive(pid: int | None) -> bool:
    if not pid or sys.platform == "win32":
        return bool(pid)
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def checkin_record(
    pid: int | None = None,
    *,
    started_at: int | None = None,
    heartbeat: int | None = None,
) -> dict:
    now = int(time.time())
    return {
        "pid": pid if pid is not None else os.getpid(),
        "host": _host(),
        "startedAt": started_at if started_at is not None else now,
        "heartbeat": heartbeat if heartbeat is not None else now,
    }


def is_checkin_stale(record: dict | None) -> bool:
    """True when the slot is free or the holder is gone / heartbeat expired."""
    if not record or not isinstance(record, dict):
        return True
    heartbeat = int(record.get("heartbeat") or 0)
    if time.time() - heartbeat > CHECKIN_STALE_SECONDS:
        return True
    pid = record.get("pid")
    if pid is not None and not _pid_alive(int(pid)):
        return True
    return False


def iter_slot_names(existing: dict | None = None) -> list[str]:
    """Ordered slot names: worker, worker-2, worker-3, ..."""
    names = ["worker"] + [f"worker-{n}" for n in range(2, MAX_AUTO_SLOTS + 1)]
    if existing:
        extra = sorted(
            k for k in existing if SLOT_NAME.match(k) and k not in names
        )
        names.extend(extra)
    return names


def load_checkins(path: Path | None = None) -> dict:
    return read_json(path or checkins_path())


def find_worker_by_pid(pid: int | None = None, path: Path | None = None) -> str | None:
    pid = os.getpid() if pid is None else pid
    for name, record in load_checkins(path).items():
        if isinstance(record, dict) and record.get("pid") == pid:
            return name
    return None


def _write_checkin(path: Path, checkins: dict, worker: str, record: dict) -> None:
    checkins[worker] = record
    atomic_write_json(path, checkins)


def acquire_checkin(explicit: str | None = None, path: Path | None = None) -> str:
    """Claim a worker slot. Explicit `--worker` / $WORKER_ID wins; else first free slot."""
    path = path or checkins_path()
    requested = explicit or os.environ.get("WORKER_ID")
    with exclusive(path.with_name(path.name + ".lock")):
        checkins = read_json(path)
        pid = os.getpid()

        if requested:
            worker = worker_name(requested)
            current = checkins.get(worker)
            if current and not is_checkin_stale(current) and current.get("pid") != pid:
                holder = current.get("pid")
                raise CheckinError(f"slot {worker!r} busy (pid {holder} on {current.get('host')})")
            started = (current or {}).get("startedAt") if current and current.get("pid") == pid else None
            _write_checkin(path, checkins, worker, checkin_record(pid, started_at=started))
            return worker

        existing = find_worker_by_pid(pid, path)
        if existing:
            record = checkins.get(existing) or checkin_record(pid)
            record = dict(record)
            record["heartbeat"] = int(time.time())
            _write_checkin(path, checkins, existing, record)
            return existing

        for worker in iter_slot_names(checkins):
            current = checkins.get(worker)
            if current and not is_checkin_stale(current) and current.get("pid") != pid:
                continue
            started = (current or {}).get("startedAt") if current and current.get("pid") == pid else None
            _write_checkin(path, checkins, worker, checkin_record(pid, started_at=started))
            return worker

    raise CheckinError("no free worker slot (all check-ins active)")


def refresh_checkin(worker: str, path: Path | None = None) -> None:
    path = path or checkins_path()
    worker = worker_name(worker)
    with exclusive(path.with_name(path.name + ".lock")):
        checkins = read_json(path)
        record = checkins.get(worker)
        if not record or record.get("pid") != os.getpid():
            raise CheckinError(f"not checked in as {worker!r}")
        record = dict(record)
        record["heartbeat"] = int(time.time())
        _write_checkin(path, checkins, worker, record)


def release_checkin(worker: str | None = None, path: Path | None = None) -> bool:
    """Release a slot held by this process. Returns True when an entry was removed."""
    path = path or checkins_path()
    pid = os.getpid()
    if worker is None:
        worker = find_worker_by_pid(pid, path)
        if worker is None:
            return False
    else:
        worker = worker_name(worker)
    with exclusive(path.with_name(path.name + ".lock")):
        checkins = read_json(path)
        record = checkins.get(worker)
        if not record or record.get("pid") != pid:
            return False
        del checkins[worker]
        atomic_write_json(path, checkins)
    return True


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
    parser.add_argument(
        "command",
        choices=["claim", "release", "list", "checkin"],
    )
    parser.add_argument(
        "checkin_action",
        nargs="?",
        choices=["acquire", "release", "heartbeat"],
        help="With command checkin: acquire | release | heartbeat",
    )
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID or auto check-in)")
    parser.add_argument("--ref", default=None, help="issue:N or pr:N")
    args = parser.parse_args()

    if args.command == "list":
        print(json.dumps(load_claims(), ensure_ascii=False, indent=2))
        return

    if args.command == "checkin":
        if not args.checkin_action:
            sys.exit("checkin requires acquire, release, or heartbeat")
        try:
            if args.checkin_action == "acquire":
                worker = acquire_checkin(args.worker)
                print(f"WORKER {worker}")
            elif args.checkin_action == "release":
                print("RELEASED" if release_checkin(args.worker) else "NOT CHECKED IN")
            else:
                if not args.worker and not os.environ.get("WORKER_ID"):
                    sys.exit("--worker or $WORKER_ID required for heartbeat")
                refresh_checkin(worker_name(args.worker))
                print("HEARTBEAT")
        except CheckinError as e:
            sys.stderr.write(f"{e}\n")
            sys.exit(2)
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
