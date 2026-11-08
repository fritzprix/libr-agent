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


def tool_result_texts(messages: list[dict]) -> list[str]:
    texts: list[str] = []
    for msg in messages:
        for block in msg.get("content") or []:
            if not isinstance(block, dict):
                continue
            if block.get("type") == "tool_result":
                text = block.get("text") or ""
                if isinstance(text, str):
                    texts.append(text)
    return texts


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


def extract_run_powershell_results(steps: list[dict]) -> list[str]:
    outputs: list[str] = []
    pattern = re.compile(r"runPowerShell|runInPersistentPowerShell", re.I)
    for step in steps:
        for call in step.get("tool_calls") or []:
            name = str(call.get("name") or call.get("tool_name") or "")
            if not pattern.search(name):
                continue
            result = call.get("result") or {}
            content = result.get("content") if isinstance(result, dict) else None
            if isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get("text"):
                        outputs.append(str(item["text"]))
    return outputs


def require_session_evidence() -> None:
    """Baseline contract: a passing trial must have evidence with a sessionId.

    (Formerly ``negative_control_stale_execution_must_fail``, which
    overstated what this checks: no stale id is involved — it only proves the
    verifier rejects a run that produced no session evidence at all.)
    """
    assert load_evidence().get("sessionId"), "missing sessionId in E2E evidence"
