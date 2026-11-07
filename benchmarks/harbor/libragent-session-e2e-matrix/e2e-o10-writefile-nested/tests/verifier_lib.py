"""Trajectory helpers for the O-10 verifier.

Slim on purpose: only what ``test_outputs.py`` here needs. The fuller
verifier library (evidence loading, approval extraction, baseline contract)
lives in the dataset-level ``shared/verifier_lib.py`` and is not duplicated
in this task.
"""

from __future__ import annotations

import json
from pathlib import Path

TRAJECTORY = Path("/logs/agent/trajectory.json")


def load_trajectory() -> dict:
    if not TRAJECTORY.is_file():
        return {}
    return json.loads(TRAJECTORY.read_text(encoding="utf-8"))


def trajectory_steps(trajectory: dict) -> list[dict]:
    return list(trajectory.get("steps") or [])
