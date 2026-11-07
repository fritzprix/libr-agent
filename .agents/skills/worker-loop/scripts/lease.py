#!/usr/bin/env python3
"""One tick's lease: its slot, its tick lock, and the token that proves both.

Acquire: a session gets at most one tick; a slot whose lock is still live is
skipped. Renew: extends both, and is also the ownership check that fences
destructive git operations and pushes, because a renewed lease cannot be taken
over during the operation it protects.
"""
from __future__ import annotations

import os
import uuid
from dataclasses import dataclass
from pathlib import Path

import tick_lock
import worker_claims
import worker_slots
import worker_state
from worker_state import Owner

# Set by a wrapper (lease_run.py --acquire) for the command it runs. The wrapper
# owns the lease; `--lock` and `--unlock` inside that command leave it in place.
INHERITED_TOKEN_ENV = "WORKER_LOOP_INHERITED_TOKEN"


def lock_path(worker: str, state_file: Path | None = None) -> Path:
    return (state_file or worker_state.state_path(worker)).parent / "tick.lock"


@dataclass(frozen=True)
class Lease:
    worker: str
    token: str
    inherited: bool = False


def inherited_lease(requested: str | None, state_file: Path | None = None) -> Lease | None:
    """The wrapper's lease if this process runs under it. A leftover or mismatched
    variable matches nothing, so that caller sees a normal lock."""
    token = os.environ.get(INHERITED_TOKEN_ENV)
    if not token:
        return None
    worker = requested or worker_slots.find_worker_by_token(token)
    if worker and is_held_by(worker, token, state_file):
        return Lease(worker, token, inherited=True)
    return None


def acquire(owner: Owner, requested: str | None = None, state_file: Path | None = None) -> Lease:
    """Take a slot and its tick lock, or raise SlotBusy."""
    token = uuid.uuid4().hex
    tried: set[str] = set()
    while True:
        worker = worker_slots.acquire_slot(owner, token, requested, exclude=tried)
        try:
            locked = tick_lock.acquire(lock_path(worker, state_file), owner, token)
        except BaseException:
            worker_slots.release_slot(worker, token)
            raise
        if locked:
            break
        worker_slots.release_slot(worker, token)
        if requested or state_file:
            raise worker_slots.SlotBusy("lock-held", f"a tick lock is live in {worker}")
        tried.add(worker)
    try:
        worker_claims.reclaim_abandoned(worker_slots.live_workers())
    except BaseException:
        release(worker, token, state_file)
        raise
    return Lease(worker, token)


def is_held_by(worker: str, token: str, state_file: Path | None = None) -> bool:
    return worker_slots.holds_slot(worker, token) and tick_lock.holds(lock_path(worker, state_file), token)


def renew(worker: str, token: str, state_file: Path | None = None) -> bool:
    """Extend the lease. False means it is lost: stop working."""
    return tick_lock.renew(lock_path(worker, state_file), token) and worker_slots.renew_slot(worker, token)


def release(worker: str, token: str, state_file: Path | None = None) -> None:
    tick_lock.release(lock_path(worker, state_file), token)
    worker_slots.release_slot(worker, token)
