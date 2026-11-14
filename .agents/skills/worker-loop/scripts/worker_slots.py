#!/usr/bin/env python3
"""Worker slot assignment: which run owns `worker`, `worker-2`, ... and its worktree.

A slot says nothing about whether a tick is executing (tick_lock.py) or which
items a worker has taken (worker_claims.py). Each record carries the session's
stable owner ID and this tick's random token. Every change happens inside one
mutex, and release and renewal require the token, so a run that lost its slot
cannot disturb the next owner.
"""
from __future__ import annotations

import os
import re
import time
from pathlib import Path

import tick_lock
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


ORPHAN_AFTER_ENV = "WORKER_LOOP_ORPHAN_SECONDS"
DEFAULT_ORPHAN_AFTER_SECONDS = 7200


def orphan_after_seconds() -> int:
    """How long a dead slot must stay silent before its work may be taken over."""
    raw = os.environ.get(ORPHAN_AFTER_ENV, "").strip()
    if not raw:
        return DEFAULT_ORPHAN_AFTER_SECONDS
    try:
        seconds = int(raw)
    except ValueError:
        raise ValueError(f"{ORPHAN_AFTER_ENV} must be a whole number of seconds, got {raw!r}") from None
    if seconds <= 0:
        raise ValueError(f"{ORPHAN_AFTER_ENV} must be positive, got {seconds}")
    return seconds


def _tick_lock_path(path: Path, name: str) -> Path:
    return path.parent / name / "tick.lock"


def _orphan(path: Path, name: str, record: object, threshold: int, now: float) -> dict | None:
    """The orphan entry for this slot, or None when its work is not abandoned.

    Orphaned: the slot record is still there (the run never unlocked), neither
    the slot nor its tick lock is live, the run has been silent for longer than
    `threshold`, and it left `current` work. A slot that unlocked cleanly has no
    record; `_pick_free_slot` already hands its `current` to the next run.
    """
    if not isinstance(record, dict) or not record or lease_is_live(record):
        return None
    if lease_is_live(tick_lock.read(_tick_lock_path(path, name))):
        return None
    renewed_at = worker_state.lease_renewed_at(record)
    if now - renewed_at <= threshold:
        return None
    current = _slot_current(path, name)
    if not isinstance(current, dict) or not current.get("ref"):
        return None
    return {
        "worker": name,
        "ref": current["ref"],
        "branch": current.get("branch"),
        "lastRenewedAt": int(renewed_at),
    }


def orphaned_slots(threshold: int | None = None, path: Path | None = None) -> list[dict]:
    """Every slot whose run died with `current` work, oldest first."""
    path = path or slots_path()
    threshold = orphan_after_seconds() if threshold is None else threshold
    now = time.time()
    found = [
        entry
        for name, record in load_slots(path).items()
        if (entry := _orphan(path, name, record, threshold, now)) is not None
    ]
    return sorted(found, key=lambda entry: entry["lastRenewedAt"])


def fence_orphan(
    name: str, taker: str, taker_token: str, threshold: int | None = None, path: Path | None = None
) -> dict | None:
    """Hand an orphaned slot to `taker` for the duration of a takeover.

    The dead run's lease is replaced by a handoff lease that only the taker
    knows, so the dead run can never renew the slot and no new tick can acquire
    it (and its `current`) until the taker calls `end_handoff`. Return the
    orphan entry plus `handoffToken` and `orphanRecord`, or None when the slot
    is no longer orphaned (it came back, was cleaned up, or another run fenced
    it first). The check and the swap happen under the slots mutex, so exactly
    one caller wins.
    """
    path = path or slots_path()
    threshold = orphan_after_seconds() if threshold is None else threshold
    handoff_token = f"{taker_token}:handoff"
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        record = slots.get(name)
        entry = _orphan(path, name, record, threshold, time.time())
        if entry is None:
            return None
        now = int(time.time())
        slots[name] = {
            "owner": f"handoff:{taker}", "scope": "handoff", "host": record.get("host"),
            "token": handoff_token, "startedAt": now, "renewedAt": now,
        }
        worker_state.atomic_write_json(path, slots)
    tick_lock.release(_tick_lock_path(path, name), record.get("token"))
    return {**entry, "handoffToken": handoff_token, "orphanRecord": {**record, "token": handoff_token}}


def end_handoff(name: str, fenced: dict, handed_over: bool, path: Path | None = None) -> None:
    """Finish a takeover started by `fence_orphan`.

    Handed over: free the slot. Otherwise put the orphan record back (with a
    token the dead run does not have), so `orphaned_slots` reports the work
    again instead of losing it.
    """
    path = path or slots_path()
    with exclusive(path.with_name(path.name + ".lock")):
        slots = read_json(path)
        if (slots.get(name) or {}).get("token") != fenced["handoffToken"]:
            return
        if handed_over:
            del slots[name]
        else:
            slots[name] = fenced["orphanRecord"]
        worker_state.atomic_write_json(path, slots)
