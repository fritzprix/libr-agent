#!/usr/bin/env python3
"""Shared foundation of worker-loop: paths, JSON files, cross-process mutex,
session identity, lease liveness, repo-wide operating state, and `status`.

worker-loop keeps three separate concepts, in three modules:

  worker_slots.py   who is assigned to a worker slot and its worktree
  tick_lock.py      whether a tick is executing in that slot
  worker_claims.py  which issue or PR a worker is working on

lease.py combines the first two for one tick. A BUSY or a claim conflict says
only what it says: see the `reason` it prints before concluding anything about
other workers.

State lives outside the user's checkout:
`<repo>/../<repo-name>.worktrees/.worker-loop/` (override with $LOOP_STATE_DIR).

CLI:
  worker_state.py claim   --worker W --ref issue:N [--token T]   0 claimed, 1 held by another worker, 2 error, 4 lease lost
  worker_state.py release --worker W --ref pr:N                  release our own claim
  worker_state.py list                                           print claims.json
  worker_state.py status                                         every slot, tick, claim, needsUser, and scheduler
  worker_state.py needs-user --ref R --question Q                record once; prints ADDED or ALREADY ASKED
  worker_state.py needs-user --ref R --clear                     remove after the user answered
  worker_state.py ci-attempt --pr N                              count one attempt; print the new total
  worker_state.py ci-attempt --pr N --clear                      reset when CI is green
  worker_state.py scheduler add --kind K --id I                  record an armed schedule
  worker_state.py scheduler list | clear                         list; clear prints what the caller must stop
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
import uuid
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

DEFAULT_WORKER = "worker"
WORKER_NAME = re.compile(r"^[a-z][a-z0-9-]{0,31}$")
# A lease nobody renewed for this long is abandoned, unless a live PID proves
# its session still exists. Long commands renew through lease_run.py.
LEASE_TTL_SECONDS = 1800
EXIT_LEASE_LOST = 4
_HOLDER_ENV = "WORKER_LOOP_HOLDER_PID"
# Checked in order. Harnesses that expose a stable per-session ID go here.
_SESSION_ENVS = ("WORKER_LOOP_SESSION_ID", "CLAUDE_CODE_SESSION_ID")
_TRANSIENT_SHELLS = frozenset({"bash", "sh", "zsh", "dash", "fish", "pwsh", "cmd"})
# Windows process-termination query (PROCESS_QUERY_LIMITED_INFORMATION).
_WIN_PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
_WIN_STILL_ACTIVE = 259  # STILL_ACTIVE is 0x00000103, not 0x100
# OpenProcess: this code means the PID does not exist. Any other error
# (including ERROR_ACCESS_DENIED = 5) means the process is alive.
_WIN_ERROR_INVALID_PARAMETER = 87


class StateFileCorrupt(RuntimeError):
    """An existing state file could not be read or parsed.

    Callers must stop rather than write: a state file we cannot read is still
    data, and saving over it would wipe it (#2612).
    """


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


def state_path(worker: str) -> Path:
    return worker_dir(worker) / "state.json"


def shared_path() -> Path:
    return state_root() / "shared.json"


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


def _proc_stat_fields(pid: int) -> list[str]:
    """Fields of /proc/PID/stat after the comm field, which may contain spaces."""
    return Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()


def _linux_ppid(pid: int) -> int:
    try:
        return int(_proc_stat_fields(pid)[1])
    except (OSError, ValueError, IndexError):
        return 0


def _read_cmdline(pid: int) -> list[str]:
    try:
        raw = Path(f"/proc/{pid}/cmdline").read_bytes()
        return [part.decode(errors="replace") for part in raw.split(b"\0") if part]
    except OSError:
        return []


def _ps(pid: int, field: str) -> str:
    """One `ps` field, for POSIX systems without /proc (macOS)."""
    try:
        result = subprocess.run(
            ["ps", "-o", f"{field}=", "-p", str(pid)], capture_output=True, text=True
        )
    except OSError:
        return ""
    return result.stdout.strip() if result.returncode == 0 else ""


def _process_info(pid: int) -> ProcessInfo | None:
    if pid <= 0:
        return None
    if _process_info_lookup is not None:
        return _process_info_lookup(pid)
    if sys.platform == "win32":
        return None
    if Path("/proc/self/stat").exists():
        try:
            comm = Path(f"/proc/{pid}/comm").read_text().strip()
        except OSError:
            return None
        return ProcessInfo(comm=comm, cmdline=_read_cmdline(pid), ppid=_linux_ppid(pid))
    comm = _ps(pid, "comm")
    if not comm:
        return None
    try:
        ppid = int(_ps(pid, "ppid") or 0)
    except ValueError:
        ppid = 0
    return ProcessInfo(comm=comm, cmdline=_ps(pid, "args").split(), ppid=ppid)


def _pid_start(pid: int) -> str | None:
    """A value that changes when `pid` is reused; None where it cannot be read."""
    if sys.platform == "win32":
        return _win_pid_start(pid)
    try:
        if Path("/proc/self/stat").exists():
            return _proc_stat_fields(pid)[19]
    except (OSError, IndexError):
        return None
    return _ps(pid, "lstart") or None


def _comm_base(comm: str) -> str:
    return comm.rsplit("/", 1)[-1].lower()


def _is_transient_shell(info: ProcessInfo) -> bool:
    base = _comm_base(info.comm)
    if base not in _TRANSIENT_SHELLS and not base.startswith("bash"):
        return False
    lowered = [arg.lower() for arg in info.cmdline]
    return "-c" in info.cmdline or "/c" in lowered


def _is_script_runner(info: ProcessInfo) -> bool:
    base = _comm_base(info.comm)
    if base.startswith("python"):
        return any(name in " ".join(info.cmdline).lower() for name in ("tick_gate", "worker_state", "lease_run"))
    return base.startswith("ruby")


def _is_app_holder(info: ProcessInfo) -> bool:
    """LibrAgent desktop: one long-lived PID shared by all of its sessions."""
    return "libragent" in " ".join(info.cmdline).lower() or _comm_base(info.comm) == "libragent"


def _is_session_holder(info: ProcessInfo) -> bool:
    """A long-lived process that exists once per agent session (Claude Code)."""
    if _comm_base(info.comm) == "claude":
        return True
    # npm-installed Claude Code runs as `node .../claude`.
    return (
        _comm_base(info.comm).startswith("node")
        and len(info.cmdline) > 1
        and _comm_base(info.cmdline[1]) == "claude"
    )


def resolve_holder(explicit: int | None = None) -> tuple[int | None, str | None]:
    """(pid, kind) of the process that outlives this tick; kind is 'session' or 'app'.

    Walks parent processes through /proc or `ps`. Windows has neither, so hosts
    there are anonymous unless they pass --pid or a session ID. Tests inject a
    lookup and exercise the walk on every platform.
    """
    env = os.environ.get(_HOLDER_ENV)
    declared = explicit if explicit is not None else (int(env) if env else None)
    if declared is not None:
        info = _process_info(declared)
        return declared, "app" if info and _is_app_holder(info) else "session"

    pid = os.getppid()
    for _ in range(40):
        info = _process_info(pid)
        if info is None:
            break
        if _is_transient_shell(info) or _is_script_runner(info):
            pid = info.ppid
            continue
        if _is_app_holder(info):
            return pid, "app"
        if _is_session_holder(info):
            return pid, "session"
        pid = info.ppid
    return None, None


def resolve_holder_pid(explicit: int | None = None) -> int | None:
    return resolve_holder(explicit)[0]


@dataclass(frozen=True)
class Owner:
    """Who holds a lease. `id` is stable across every command of one session.

    scope 'session': one agent session; a second tick from it is a duplicate.
    scope 'app':     a process shared by many sessions; duplicates cannot be told apart.
    scope 'anon':    nothing identifies the session; the tick token is all we have.
    pid_bound:       the PID lives exactly as long as the session, so a live PID
                     proves the session exists however long ago it last renewed.
    """

    id: str
    scope: str
    pid: int | None = None
    pid_start: str | None = None
    pid_bound: bool = False

    def record(self) -> dict:
        record: dict = {"owner": self.id, "scope": self.scope, "host": _host()}
        if self.pid is not None:
            record["pid"] = self.pid
            record["pidBound"] = self.pid_bound
            if self.pid_start:
                record["pidStart"] = self.pid_start
        return record


def resolve_owner(explicit_pid: int | None = None, session: str | None = None) -> Owner:
    pid, kind = resolve_holder(explicit_pid)
    start = _pid_start(pid) if pid else None
    bound = kind == "session"
    session = session or next((os.environ[e] for e in _SESSION_ENVS if os.environ.get(e)), None)
    if session:
        return Owner(f"session:{session}", "session", pid, start, bound)
    if kind == "session":
        return Owner(f"pid:{pid}:{start}", "session", pid, start, True)
    if kind == "app":
        return Owner(f"app:{pid}:{start}", "app", pid, start, False)
    return Owner(f"anon:{uuid.uuid4().hex}", "anon")


def _win_pid_start(pid: int) -> str | None:
    """The process creation time from GetProcessTimes; None if it cannot be read."""
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel32.GetProcessTimes.argtypes = (wintypes.HANDLE, *[ctypes.POINTER(wintypes.FILETIME)] * 4)
    kernel32.CloseHandle.argtypes = (wintypes.HANDLE,)
    handle = kernel32.OpenProcess(_WIN_PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return None
    try:
        created, exited, kernel, user = (wintypes.FILETIME() for _ in range(4))
        if not kernel32.GetProcessTimes(
            handle,
            ctypes.byref(created),
            ctypes.byref(exited),
            ctypes.byref(kernel),
            ctypes.byref(user),
        ):
            return None
        return str((created.dwHighDateTime << 32) | created.dwLowDateTime)
    finally:
        kernel32.CloseHandle(handle)


def _win_pid_alive(pid: int) -> bool:
    """Windows PID liveness probe via kernel32 (there is no os.kill(pid, 0)).

    A failed OpenProcess is not proof the PID is dead. ERROR_INVALID_PARAMETER
    (87) means the PID does not exist. Any other error, including
    ERROR_ACCESS_DENIED (5), means the process is alive but not queryable.
    """
    import ctypes
    from ctypes import wintypes

    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.OpenProcess.argtypes = (wintypes.DWORD, wintypes.BOOL, wintypes.DWORD)
    kernel32.GetExitCodeProcess.argtypes = (wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD))
    handle = kernel32.OpenProcess(
        _WIN_PROCESS_QUERY_LIMITED_INFORMATION, False, pid
    )
    if not handle:
        return ctypes.get_last_error() != _WIN_ERROR_INVALID_PARAMETER
    try:
        exit_code = wintypes.DWORD()
        if not kernel32.GetExitCodeProcess(handle, ctypes.byref(exit_code)):
            return True  # cannot query: assume alive, TTL will eventually free the slot
        return exit_code.value == _WIN_STILL_ACTIVE
    finally:
        kernel32.CloseHandle(handle)


def _pid_alive(pid: int | None) -> bool:
    if not pid:
        return False
    if sys.platform == "win32":
        return _win_pid_alive(int(pid))
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def lease_expired(record: dict) -> bool:
    last = record.get("renewedAt") or record.get("startedAt") or record.get("time") or 0
    return time.time() - float(last) > LEASE_TTL_SECONDS


def lease_is_live(record: dict | None) -> bool:
    """Is the run that wrote this slot or lock record still entitled to it?

    A dead PID ends it. A live session-bound PID keeps it however old: a running
    session's work is never taken by anyone else. Everything else needs renewal.
    """
    if not isinstance(record, dict) or not record:
        return False
    pid = record.get("pid")
    if pid:
        if not _pid_alive(int(pid)):
            return False
        started = record.get("pidStart")
        if started and _pid_start(int(pid)) not in (None, started):
            return False  # the PID was reused by another process
        if record.get("pidBound"):
            return True
    return not lease_expired(record)


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
    """Read a JSON object file. A missing file is empty; an existing file that
    cannot be read, parsed, or is not an object raises StateFileCorrupt so that
    no caller ever saves over data it failed to understand (#2612).

    utf-8-sig also strips a UTF-8 BOM, e.g. when the file was rewritten by
    PowerShell 5.1 `Set-Content -Encoding UTF8`.
    """
    if not path.is_file():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError) as e:
        raise StateFileCorrupt(f"{path} is not readable JSON: {e}") from e
    if not isinstance(data, dict):
        raise StateFileCorrupt(f"{path} does not contain a JSON object")
    return data


def _try_os_lock(fd: int) -> None:
    if sys.platform == "win32":
        import msvcrt

        os.lseek(fd, 0, os.SEEK_SET)
        msvcrt.locking(fd, msvcrt.LK_NBLCK, 1)
    else:
        import fcntl

        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)


def _os_unlock(fd: int) -> None:
    if sys.platform == "win32":
        import msvcrt

        os.lseek(fd, 0, os.SEEK_SET)
        msvcrt.locking(fd, msvcrt.LK_UNLCK, 1)
    # POSIX: closing the descriptor drops the flock.


@contextmanager
def exclusive(lock_path: Path, timeout: float = 10.0):
    """Cross-process mutex for one read-modify-write of a shared file.

    An OS lock on a file that is never deleted: the kernel drops it when the
    holder dies, so there is no stale state to break, and breaking a stale lock
    can never let two writers in.
    """
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(lock_path, os.O_RDWR | os.O_CREAT, 0o644)
    deadline = time.monotonic() + timeout
    try:
        while True:
            try:
                _try_os_lock(fd)
                break
            except OSError:
                if time.monotonic() > deadline:
                    raise TimeoutError(f"could not acquire {lock_path}") from None
                time.sleep(0.01)
        try:
            yield
        finally:
            _os_unlock(fd)
    finally:
        os.close(fd)


# --- Repo-wide operating state ------------------------------------------------
# needsUser, ciAttempts, and schedulers describe the repo, not a slot: any worker
# must see them, and `status` and `stop` must find them whichever slot armed them.


def _load_shared(path: Path) -> dict:
    """Read shared.json; on first use, adopt what older versions kept per slot."""
    if path.is_file():
        return read_json(path)
    shared: dict = {"needsUser": [], "ciAttempts": {}, "schedulers": []}
    for slot_state in sorted(path.parent.glob("*/state.json")):
        state = read_json(slot_state)
        known = {item.get("ref") for item in shared["needsUser"]}
        shared["needsUser"] += [i for i in state.get("needsUser") or [] if i.get("ref") not in known]
        for pr, count in (state.get("ciAttempts") or {}).items():
            shared["ciAttempts"][pr] = max(count, shared["ciAttempts"].get(pr, 0))
        if state.get("scheduler"):
            shared["schedulers"].append(state["scheduler"])
    return shared


@contextmanager
def _shared_update():
    path = shared_path()
    with exclusive(path.with_name(path.name + ".lock")):
        shared = _load_shared(path)
        yield shared
        atomic_write_json(path, shared)


def add_needs_user(ref: str, question: str) -> bool:
    """Record a question once across all workers. False means it was already asked."""
    with _shared_update() as shared:
        if any(item.get("ref") == ref for item in shared["needsUser"]):
            return False
        since = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        shared["needsUser"].append({"ref": ref, "question": question, "since": since})
    return True


def clear_needs_user(ref: str) -> None:
    with _shared_update() as shared:
        shared["needsUser"] = [i for i in shared["needsUser"] if i.get("ref") != ref]


def count_ci_attempt(pr: int) -> int:
    with _shared_update() as shared:
        count = shared["ciAttempts"].get(str(pr), 0) + 1
        shared["ciAttempts"][str(pr)] = count
    return count


def clear_ci_attempts(pr: int) -> None:
    with _shared_update() as shared:
        shared["ciAttempts"].pop(str(pr), None)


def add_scheduler(kind: str, scheduler_id: str) -> None:
    with _shared_update() as shared:
        entry = {"kind": kind, "id": scheduler_id}
        if entry not in shared["schedulers"]:
            shared["schedulers"].append(entry)


def list_schedulers() -> list[dict]:
    return _load_shared(shared_path())["schedulers"]


def clear_schedulers() -> list[dict]:
    """Forget all schedules and return them: the caller must stop each one."""
    with _shared_update() as shared:
        cleared, shared["schedulers"] = shared["schedulers"], []
    return cleared


def status() -> dict:
    """Everything an operator needs, for every worker that left a trace."""
    import tick_lock
    import worker_claims
    import worker_slots

    root = state_root()
    slots = worker_slots.load_slots()
    names = set(slots) | {p.parent.name for p in root.glob("*/state.json")}
    claims = worker_claims.load_claims()
    workers = {}
    for name in sorted(names):
        state = read_json(root / name / "state.json")
        lock = tick_lock.read(root / name / "tick.lock")
        workers[name] = {
            "slot": slots.get(name),
            "slotLive": lease_is_live(slots.get(name)),
            "tickRunning": lease_is_live(lock),
            "current": state.get("current"),
            "claims": sorted(r for r, c in claims.items() if c.get("worker") == name),
        }
    shared = _load_shared(shared_path())
    return {"workers": workers, **shared}


def main() -> None:
    import worker_claims
    import worker_slots

    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "command",
        choices=["claim", "release", "list", "status", "needs-user", "ci-attempt", "scheduler"],
    )
    parser.add_argument("action", nargs="?", choices=["add", "list", "clear"], help="With scheduler")
    parser.add_argument("--worker", default=None, help="Worker name (default: $WORKER_ID)")
    parser.add_argument("--token", default=None, help="Tick token from `tick_gate.py --lock`")
    parser.add_argument("--ref", default=None, help="issue:N or pr:N")
    parser.add_argument("--question", default=None)
    parser.add_argument("--pr", type=int, default=None)
    parser.add_argument("--clear", action="store_true")
    parser.add_argument("--kind", default=None)
    parser.add_argument("--id", default=None)
    args = parser.parse_args()

    # Exit 1 means only "another worker holds it"; every failure here is 2, so
    # an unreadable claims file is never mistaken for a held claim.
    try:
        if args.command == "list":
            print(json.dumps(worker_claims.load_claims(), ensure_ascii=False, indent=2))
        elif args.command == "status":
            print(json.dumps(status(), ensure_ascii=False, indent=2))
        elif args.command == "needs-user":
            if args.clear:
                clear_needs_user(args.ref)
                print("CLEARED")
            else:
                print("ADDED" if add_needs_user(args.ref, args.question) else "ALREADY ASKED")
        elif args.command == "ci-attempt":
            if args.clear:
                clear_ci_attempts(args.pr)
                print("CLEARED")
            else:
                print(count_ci_attempt(args.pr))
        elif args.command == "scheduler":
            if args.action == "add":
                add_scheduler(args.kind, args.id)
                print("ADDED")
            else:
                entries = clear_schedulers() if args.action == "clear" else list_schedulers()
                print(json.dumps(entries, ensure_ascii=False, indent=2))
        else:
            if not args.ref:
                raise ValueError("--ref is required")
            worker = worker_name(args.worker)
            if args.command == "release":
                print("RELEASED" if worker_claims.release(worker, args.ref) else "NOT HELD")
                return
            if args.token and not worker_slots.holds_slot(worker, args.token):
                print(f"LEASE LOST: {worker} no longer holds this tick; stop working")
                sys.exit(EXIT_LEASE_LOST)
            holder = worker_claims.claim(worker, args.ref)
            if holder:
                print(f"HELD by {holder}")
                sys.exit(1)
            print(f"CLAIMED {args.ref} for {worker}")
    except (StateFileCorrupt, TimeoutError, OSError, ValueError, TypeError, KeyError) as e:
        sys.stderr.write(f"worker_state error: {e}\n")
        sys.exit(2)


if __name__ == "__main__":
    # Run through the importable module, so the exception classes here are the
    # same ones worker_claims and worker_slots raise.
    import worker_state as _module

    _module.main()
