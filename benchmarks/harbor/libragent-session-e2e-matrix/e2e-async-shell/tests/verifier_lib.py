"""Minimal verifier helpers for e2e-async-shell."""

from __future__ import annotations

import json
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")


def load_evidence() -> dict:
    if not EVIDENCE.is_file():
        return {}
    return json.loads(EVIDENCE.read_text(encoding="utf-8"))


def load_trajectory() -> dict:
    if not TRAJECTORY.is_file():
        return {}
    return json.loads(TRAJECTORY.read_text(encoding="utf-8"))


def trajectory_steps(trajectory: dict) -> list[dict]:
    return list(trajectory.get("steps") or [])


def assert_no_synthetic_literal_in_transcript(
    blob: str, *, literal: str, allow_name: str
) -> None:
    """Fail if the synthetic secret literal appears; variable names are allowed."""
    if not literal:
        return
    if literal in blob:
        raise AssertionError("synthetic secret literal leaked in transcript")
    if allow_name and allow_name in blob:
        return


def require_session_evidence() -> None:
    """Baseline contract: a passing trial must have evidence with a sessionId."""
    assert load_evidence().get("sessionId"), "missing sessionId in E2E evidence"
