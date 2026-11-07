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


def observation_result_texts(steps: list[dict]) -> list[str]:
    """Text of every tool result, from the ATIF ``observation`` blocks.

    The exported trajectory stores tool results as
    ``step["observation"]["results"][i]["content"]`` (a plain string), not on
    the tool call itself — reading the latter silently yields nothing.
    """
    texts: list[str] = []
    for step in steps:
        observation = step.get("observation") or {}
        for result in observation.get("results") or []:
            content = result.get("content")
            if isinstance(content, str) and content:
                texts.append(content)
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("text"):
                        texts.append(str(item["text"]))
    return texts
