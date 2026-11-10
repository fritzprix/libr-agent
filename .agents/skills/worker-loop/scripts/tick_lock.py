#!/usr/bin/env python3
"""Tick execution lock: is a tick running in this slot right now?

Check-and-write, renewal, and release all happen inside an OS mutex on a sibling
guard file, and every lock carries a random token. Release and renewal compare
the token, so a run that lost the lock can never remove or extend a newer
owner's lock.
"""
from __future__ import annotations

import json
import time
import uuid
from pathlib import Path

import worker_state
from worker_state import Owner, exclusive, lease_is_live


def _guard(lock: Path) -> Path:
    return lock.with_name(lock.name + ".guard")


def read(lock: Path) -> dict | None:
    """The lock record; None when absent. A corrupt lock is reported as an empty
    record, which is not live: writes are atomic, so nobody is mid-acquire."""
    if not lock.is_file():
        return None
    try:
        data = json.loads(lock.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return {}
    return data if isinstance(data, dict) else {}


def is_held(lock: Path) -> bool:
    return lease_is_live(read(lock))


def _is_own_session(record: dict | None, owner: Owner | None) -> bool:
    """Does this lock belong to the session now acquiring it?

    The slot layer has already refused a session whose tick is still running, so
    such a lock is an abandoned tick. A live PID-bound lock never expires by age:
    without this, the session could never replace it and the slot would stay
    blocked until the process exits. App and anonymous IDs can be shared by
    concurrent runs, so they do not qualify.
    """
    return bool(record and owner and owner.scope == "session" and record.get("owner") == owner.id)


def acquire(lock: Path, owner: Owner | None, token: str | None = None, pid: int | None = None) -> str | None:
    """Take the lock atomically. Return its token, or None while another live run holds it."""
    token = token or uuid.uuid4().hex
    with exclusive(_guard(lock)):
        record = read(lock)
        if lease_is_live(record) and not _is_own_session(record, owner):
            return None
        now = int(time.time())
        record = {**(owner.record() if owner else {}), "token": token, "time": now, "renewedAt": now}
        if pid is not None:
            record["pid"] = pid
        worker_state.atomic_write_json(lock, record)
    return token


def holds(lock: Path, token: str) -> bool:
    return (read(lock) or {}).get("token") == token


def renew(lock: Path, token: str) -> bool:
    with exclusive(_guard(lock)):
        record = read(lock)
        if not record or record.get("token") != token:
            return False
        record["renewedAt"] = int(time.time())
        worker_state.atomic_write_json(lock, record)
    return True


def release(lock: Path, token: str | None) -> bool:
    """Remove the lock if `token` owns it. No token is the operator's force path."""
    with exclusive(_guard(lock)):
        record = read(lock)
        if record is None or (token is not None and record.get("token") != token):
            return False
        lock.unlink(missing_ok=True)
    return True
