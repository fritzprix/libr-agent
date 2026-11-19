"""Evidence + trajectory helpers for the two-session-prompts verifier.

Task-local on purpose: Harbor mounts each task's ``tests/`` directory on its
own, so this copy is the import path the verifier can resolve. The fuller
library lives in the dataset-level ``shared/verifier_lib.py``.
"""

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


def injected_values(evidence: dict) -> list[str]:
    """Exact values the harness recorded as answers it injected (``injectedValue``).

    The host harness reads ``LIBRAGENT_E2E_SYNTHETIC_SECRET`` and records the
    value it sent in ``session_e2e_evidence.json``; this verifier container
    cannot see that env var, so the evidence record — not an env guess — is
    what keeps leak checks non-vacuous when CI sets a different secret
    (gatekeeper G3.1).
    """
    return [
        str(action["injectedValue"])
        for action in evidence.get("resolvedInteractions") or []
        if action.get("injectedValue")
    ]


def trajectory_steps(trajectory: dict) -> list[dict]:
    return list(trajectory.get("steps") or [])


def observation_result_texts(steps: list[dict]) -> list[str]:
    """Text of every tool result, from the ATIF ``observation`` blocks."""
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
