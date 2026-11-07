"""Independent graders for Session API E2E Harbor tasks."""

from __future__ import annotations

import json
import re
from pathlib import Path

EVIDENCE = Path("/logs/agent/session_e2e_evidence.json")
TRAJECTORY = Path("/logs/agent/trajectory.json")
WORKSPACE = Path("/app")


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
