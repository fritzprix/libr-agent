#!/usr/bin/env python3
"""Per-worker state paths, atomic JSON writes, shared claims, and session check-ins.

Several workers (worker, worker-2, ...) may run in parallel, each in its own
worktree. Each worker has its own directory with `state.json` and `tick.lock`.
`claims.json` is shared: a claim on `issue:N` or `pr:N` names the worker that
owns it, so two workers never take the same issue or PR.

When no `--worker` / $WORKER_ID is set, a session checks in under `checkins.json`
and receives the first free slot (`worker`, `worker-2`, ...). When a long-lived
holder PID is known (e.g. LibrAgent desktop), it is stored; otherwise the slot
uses a TTL until `--unlock` (no bogus transient bash/node PID).

State lives outside the user's checkout:
`<repo>/../<repo-name>.worktrees/.worker-loop/` (override with $LOOP_STATE_DIR).

CLI:
  worker_state.py claim   --worker W --ref issue:N   exit 0 claimed (or already ours), 1 held by another worker
  worker_state.py release --worker W --ref pr:N      release our own claim
  worker_state.py list                               print claims.json
  worker_state.py checkin acquire [--worker W] [--holder-pid P]   claim a slot; print WORKER line
  worker_state.py checkin release [--worker W] [--holder-pid P]     release our slot
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
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

DEFAULT_WORKER = "worker"
WORKER_NAME = re.compile(r"^[a-z][a-z0-9-]{0,31}$")
REF_FORMAT = re.compile(r"^(issue|pr):\d+$")
SLOT_NAME = re.compile(r"^worker(?:-\d+)?$")
# A crashed writer must not block claims forever; claim updates take milliseconds.
CLAIMS_LOCK_STALE_SECONDS = 60
# Windows has no reliable pid liveness probe; match tick_gate LOCK_TTL_SECONDS.
CHECKIN_TTL_SECONDS = 1800
MAX_AUTO_SLOTS = 32
_HOLDER_ENV = "WORKER_LOOP_HOLDER_PID"
_TRANSIENT_SHELLS = frozenset({"bash", "sh", "zsh", "dash", "fish", "pwsh", "cmd"})
_HOLDER_UNSET = object()


class CheckinError(RuntimeError):
    """Could not acquire or release a worker check-in slot."""


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


@dataclass(frozen=True)
class ProcessInfo:
    comm: str
    cmdline: list[str]
    ppid: int


# Tests inject a pid → ProcessInfo map here (see test_worker_state.py).
_process_info_lookup: Callable[[int], ProcessInfo | None] | None = None


def _linux_ppid(pid: int) -> int:
    try:
        return int(Path(f"/proc/{pid}/stat").read_text().split()[3])
    except (OSError, ValueError, IndexError):
        return 0


def _read_cmdline(pid: int) -> list[str]:
    try:
        raw = Path(f"/proc/{pid}/cmdline").read_bytes()
        return [part.decode(errors="replace") for part in raw.split(b"\0") if part]
    except OSError:
        return []


def _process_info(pid: int) -> ProcessInfo | None:
    if pid <= 0:
        return None
    if _process_info_lookup is not None:
        return _process_info_lookup(pid)
    try:
        comm = Path(f"/proc/{pid}/comm").read_text().strip()
    except OSError:
        return None
    return ProcessInfo(comm=comm, cmdline=_read_cmdline(pid), ppid=_linux_ppid(pid))


def _comm_base(comm: str) -> str:
    return comm.rsplit("/", 1)[-1].lower()


def _is_transient_shell(info: ProcessInfo) -> bool:
    base = _comm_base(info.comm)
    if base not in _TRANSIENT_SHELLS and not base.startswith("bash"):
        return False
    lowered = [arg.lower() for arg in info.cmdline]
    return "-c" in info.cmdline or "/c" in lowered


def _is_ephemeral_runner(info: ProcessInfo) -> bool:
    base = _comm_base(info.comm)
    joined = " ".join(info.cmdline).lower()
    if base.startswith("python"):
        return "tick_gate" in joined or "worker_state" in joined
    if base.startswith("node"):
        return True
    return base.startswith("ruby")


def _is_trusted_session_holder(info: ProcessInfo) -> bool:
    joined = " ".join(info.cmdline).lower()
    base = _comm_base(info.comm)
    return "libragent" in joined or base == "libragent"


def resolve_holder_pid(explicit: int | None = None) -> int | None:
    """Long-lived session PID, or None → check-in uses TTL only (no pid field)."""
    if explicit is not None:
        return explicit
    env = os.environ.get(_HOLDER_ENV)
    if env:
        return int(env)
    if sys.platform == "win32":
        return None

    pid = os.getppid()
    for _ in range(40):
        info = _process_info(pid)
        if info is None:
            break
        if _is_transient_shell(info):
            pid = info.ppid
            continue
        if _is_ephemeral_runner(info):
            pid = info.ppid
            continue
        if _is_trusted_session_holder(info):
            return pid
        pid = info.ppid
    return None


def _pid_alive(pid: int | None) -> bool:
    if not pid:
        return False
    if sys.platform == "win32":
        return True
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def checkin_record(holder_pid: int | None, *, started_at: int | None = None) -> dict:
    now = int(time.time())
    record: dict = {
        "host": _host(),
        "startedAt": started_at if started_at is not None else now,
    }
    if holder_pid is not None:
        record["pid"] = holder_pid
    return record


def is_checkin_stale(record: dict | None) -> bool:
    """True when the slot is free, TTL expired, or (if recorded) holder PID is dead."""
    if not record or not isinstance(record, dict):
        return True
    started = int(record.get("startedAt") or 0)
    pid = record.get("pid")
    if pid is None:
        return time.time() - started > CHECKIN_TTL_SECONDS
    if sys.platform == "win32":
        return time.time() - started > CHECKIN_TTL_SECONDS
    if not _pid_alive(int(pid)):
        return True
    return False


def _same_checkin_holder(record: dict, holder: int | None) -> bool:
    rec_pid = record.get("pid")
    if holder is None:
        return rec_pid is None
    return rec_pid == holder


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


def find_worker_by_holder_pid(holder_pid: int | None, path: Path | None = None) -> str | None:
    if holder_pid is None:
        return None
    for name, record in load_checkins(path).items():
        if isinstance(record, dict) and record.get("pid") == holder_pid:
            return name
    return None


def _write_checkin(path: Path, checkins: dict, worker: str, record: dict) -> None:
    checkins[worker] = record
    atomic_write_json(path, checkins)


def acquire_checkin(
    explicit: str | None = None,
    path: Path | None = None,
    *,
    holder: int | None | object = _HOLDER_UNSET,
) -> str:
    """Claim a worker slot. Explicit `--worker` / $WORKER_ID wins; else first free slot."""
    path = path or checkins_path()
    if holder is _HOLDER_UNSET:
        resolved: int | None = resolve_holder_pid(None)
    else:
        resolved = holder  # type: ignore[assignment]
    requested = explicit or os.environ.get("WORKER_ID")
    with exclusive(path.with_name(path.name + ".lock")):
        checkins = read_json(path)

        if requested:
            worker = worker_name(requested)
            current = checkins.get(worker)
            if current and not is_checkin_stale(current) and not _same_checkin_holder(current, resolved):
                other = current.get("pid")
                raise CheckinError(f"slot {worker!r} busy (pid {other} on {current.get('host')})")
            started = (
                (current or {}).get("startedAt")
                if current and _same_checkin_holder(current, resolved)
                else None
            )
            _write_checkin(path, checkins, worker, checkin_record(resolved, started_at=started))
            return worker

        for worker in iter_slot_names(checkins):
            current = checkins.get(worker)
            if current and not is_checkin_stale(current):
                continue
            _write_checkin(path, checkins, worker, checkin_record(resolved))
            return worker

    raise CheckinError("no free worker slot (all check-ins active)")


def release_checkin(
    worker: str | None = None,
    path: Path | None = None,
    *,
    holder: int | None | object = _HOLDER_UNSET,
) -> bool:
    """Release a slot. TTL slots release by worker name; PID slots match `holder`."""
    path = path or checkins_path()
    if holder is _HOLDER_UNSET:
        resolved: int | None = resolve_holder_pid(None)
    else:
        resolved = holder  # type: ignore[assignment]

    if worker is None:
        worker = find_worker_by_holder_pid(resolved, path)
        if worker is None:
            return False
    else:
        worker = worker_name(worker)

    with exclusive(path.with_name(path.name + ".lock")):
        checkins = read_json(path)
        record = checkins.get(worker)
        if not record:
            return False
        rec_pid = record.get("pid")
        if rec_pid is None:
            del checkins[worker]
            atomic_write_json(path, checkins)
            return True
        if resolved is not None and rec_pid == resolved:
            del checkins[worker]
            atomic_write_json(path, checkins)
            return True
    return False


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
        # utf-8-sig also strips a UTF-8 BOM, e.g. when a state file was
        # rewritten by PowerShell 5.1 `Set-Content -Encoding UTF8`. Returning
        # {} on a readable file would let the next save wipe it (#2612).
        data = json.loads(path.read_text(encoding="utf-8-sig"))
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
        choices=["acquire", "release"],
        help="With command checkin: acquire | release",
    )
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID or auto check-in)")
    parser.add_argument("--holder-pid", type=int, default=None, help="PID of the tick session (default: auto-detect)")
    parser.add_argument("--ref", default=None, help="issue:N or pr:N")
    args = parser.parse_args()

    if args.command == "list":
        print(json.dumps(load_claims(), ensure_ascii=False, indent=2))
        return

    if args.command == "checkin":
        if not args.checkin_action:
            sys.exit("checkin requires acquire or release")
        try:
            resolved = resolve_holder_pid(args.holder_pid)
            if args.checkin_action == "acquire":
                worker = acquire_checkin(args.worker, holder=resolved)
                print(f"WORKER {worker}")
            else:
                print(
                    "RELEASED"
                    if release_checkin(args.worker, holder=resolved)
                    else "NOT CHECKED IN"
                )
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
