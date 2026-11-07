#!/usr/bin/env python3
"""Worker slot assignment: which run owns `worker`, `worker-2`, ... and its worktree.

A slot says nothing about whether a tick is executing (tick_lock.py) or which
items a worker has taken (worker_claims.py). Each record carries the session's
stable owner ID and this tick's random token. Every change happens inside one
mutex, and release and renewal require the token, so a run that lost its slot
cannot disturb the next owner.
"""
from __future__ import annotations

import re
import time
from pathlib import Path

import worker_state
from worker_state import Owner, exclusive, lease_expired, lease_is_live, read_json

SLOT_NAME = re.compile(r"^worker(?:-\d+)?$")
MAX_AUTO_SLOTS = 32


class SlotBusy(RuntimeError):
    """No slot for this tick. `reason` is one of:

    same-session: this session already has a tick running (duplicate tick)
    slot-held:    the requested slot belongs to another live run
    lock-held:    the slot is free but a tick lock is still live in it
    no-free-slot: every slot belongs to a live run
    """

    def __init__(self, reason: str, detail: str = ""):
        super().__init__(detail or reason)
        self.reason = reason


def slots_path() -> Path:
    # The file name predates the slot/lock split; existing state keeps working.
    return worker_state.state_root() / "checkins.json"


def load_slots(path: Path | None = None) -> dict:
    return read_json(path or slots_path())


def iter_slot_names(existing: dict | None = None) -> list[str]:
    """Ordered slot names: worker, worker-2, worker-3, ..."""
    names = ["worker"] + [f"worker-{n}" for n in range(2, MAX_AUTO_SLOTS + 1)]
    if existing:
        names.extend(sorted(k for k in existing if SLOT_NAME.match(k) and k not in names))
    return names


def _slot_current(path: Path, name: str) -> object:
    """The slot's saved `current`. A corrupt state file blocks only its own slot."""
    try:
        return read_json(path.parent / name / "state.json").get("current")
    except worker_state.StateFileCorrupt:
        return None


def _pick_free_slot(path: Path, slots: dict, exclude: set[str]) -> str:
    """First free slot, preferring one whose `current` work waits to be resumed.

    Otherwise a solo session always gets `worker`, and work left in `worker-2`
    would never be picked up again.
    """
    free = [
        name for name in iter_slot_names(slots)
        if name not in exclude and not lease_is_live(slots.get(name))
    ]
    for name in free:
        if _slot_current(path, name):
            return name
    if not free:
        raise SlotBusy("no-free-slot", f"all {MAX_AUTO_SLOTS} slots belong to live runs")
    return free[0]


def acquire_slot(
    owner: Owner,
    token: str,
    requested: str | None = None,
    exclude: set[str] | None = None,
    path: Path | None = None,
) -> str:
    """Give this tick a slot, or raise SlotBusy."""
    path = path or slots_path()
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        chosen = None
        if owner.scope == "session":
            for name, record in slots.items():
                if record.get("owner") != owner.id or not lease_is_live(record):
                    continue
                if requested and name != requested:
                    continue  # pinning another slot is how one session runs two workers
                if not lease_expired(record):
                    raise SlotBusy("same-session", f"this session's tick is running in {name}")
                chosen = name  # our own abandoned tick: take it back with a new token
                break
        if chosen is None and requested:
            chosen = worker_state.worker_name(requested)
            if lease_is_live(slots.get(chosen)):
                raise SlotBusy("slot-held", f"{chosen} belongs to {slots[chosen].get('owner')}")
        if chosen is None:
            chosen = _pick_free_slot(path, slots, exclude or set())
        now = int(time.time())
        slots[chosen] = {**owner.record(), "token": token, "startedAt": now, "renewedAt": now}
        worker_state.atomic_write_json(path, slots)
        return chosen


def holds_slot(worker: str, token: str, path: Path | None = None) -> bool:
    return (load_slots(path).get(worker) or {}).get("token") == token


def renew_slot(worker: str, token: str, path: Path | None = None) -> bool:
    path = path or slots_path()
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        record = slots.get(worker)
        if not record or record.get("token") != token:
            return False
        record["renewedAt"] = int(time.time())
        worker_state.atomic_write_json(path, slots)
    return True


def release_slot(worker: str, token: str, path: Path | None = None) -> bool:
    """Free the slot if `token` still owns it."""
    path = path or slots_path()
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        if (slots.get(worker) or {}).get("token") != token:
            return False
        del slots[worker]
        worker_state.atomic_write_json(path, slots)
    return True


def force_release_slot(worker: str, path: Path | None = None) -> None:
    """Operator recovery only: free a slot without proving ownership."""
    path = path or slots_path()
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        if slots.pop(worker, None) is not None:
            worker_state.atomic_write_json(path, slots)


def find_worker_by_token(token: str, path: Path | None = None) -> str | None:
    for name, record in load_slots(path).items():
        if isinstance(record, dict) and record.get("token") == token:
            return name
    return None


def live_workers(path: Path | None = None) -> set[str]:
    return {name for name, record in load_slots(path).items() if lease_is_live(record)}
