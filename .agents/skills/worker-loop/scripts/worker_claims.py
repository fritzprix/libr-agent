#!/usr/bin/env python3
"""Item claims: which worker is working on `issue:N` or `pr:N`.

A claim lasts for one tick. Unlock releases the worker's claims except the ref
in its `current`, which stays claimed until that work is resumed and turned into
a PR. A claim left by a crash is reclaimed at the next tick start: its worker has
no live lease and no `current` for it. Live workers' claims are never touched,
and no decision here depends on capped GitHub query results (see #2623).
"""
from __future__ import annotations

import re
import time
from pathlib import Path

import worker_state
from worker_state import exclusive, read_json

REF_FORMAT = re.compile(r"^(issue|pr):\d+$")


def claims_path() -> Path:
    return worker_state.state_root() / "claims.json"


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
        worker_state.atomic_write_json(path, claims)
    return None


def transfer(ref: str, from_worker: str, to_worker: str, path: Path | None = None) -> str | None:
    """Move `ref` from an orphaned worker to `to_worker`.

    Returns None on success, else the third worker holding it. An unclaimed ref
    is claimed for `to_worker`.
    """
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        holder = (claims.get(ref) or {}).get("worker")
        if holder and holder not in (from_worker, to_worker):
            return holder
        claims[ref] = {"worker": to_worker, "at": int(time.time())}
        worker_state.atomic_write_json(path, claims)
    return None


def release(worker: str, ref: str, path: Path | None = None) -> bool:
    """Release `ref` if `worker` holds it. Returns True when a claim was removed."""
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        if (claims.get(ref) or {}).get("worker") != worker:
            return False
        del claims[ref]
        worker_state.atomic_write_json(path, claims)
    return True


def claimed_by_other(claims: dict, ref: str, worker: str) -> str | None:
    holder = (claims.get(ref) or {}).get("worker")
    return holder if holder and holder != worker else None


def _survives_without_lease(ref: str, state: dict) -> bool:
    """Does the worker's saved `current` still need this claim?"""
    current = state.get("current")
    if not current:
        return False
    current_ref = current.get("ref") if isinstance(current, dict) else current
    # A `current` that names no ref cannot be matched: keep rather than guess.
    return current_ref == ref or not (isinstance(current_ref, str) and REF_FORMAT.match(current_ref))


def release_finished(worker: str, state: dict, path: Path | None = None) -> list[str]:
    """End of tick: drop this worker's claims that its `current` does not need."""
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        dropped = [
            ref for ref, entry in claims.items()
            if entry.get("worker") == worker and not _survives_without_lease(ref, state)
        ]
        for ref in dropped:
            del claims[ref]
        if dropped:
            worker_state.atomic_write_json(path, claims)
    return dropped


def _claim_worker(entry: object) -> str | None:
    """The worker a claim entry names, or None when the entry is malformed."""
    worker = entry.get("worker") if isinstance(entry, dict) else None
    if not worker or not isinstance(worker, str):
        return None
    try:
        return worker_state.worker_name(worker)
    except ValueError:
        return None


def reclaim_abandoned(live: set[str], path: Path | None = None) -> list[str]:
    """Tick start: drop claims of workers that have no live lease and no use for them."""
    path = path or claims_path()
    with exclusive(path.with_name(path.name + ".lock")):
        claims = read_json(path)
        dropped = []
        for ref, entry in claims.items():
            worker = _claim_worker(entry)
            if worker is None:
                dropped.append(ref)  # names no worker, so it can only block ticks
                continue
            if worker in live:
                continue
            try:
                state = read_json(path.parent / worker / "state.json")
            except worker_state.StateFileCorrupt:
                continue  # cannot prove the claim is unused
            if not _survives_without_lease(ref, state):
                dropped.append(ref)
        for ref in dropped:
            del claims[ref]
        if dropped:
            worker_state.atomic_write_json(path, claims)
    return dropped
