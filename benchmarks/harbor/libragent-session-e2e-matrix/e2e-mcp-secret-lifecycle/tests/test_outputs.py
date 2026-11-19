"""2565 M-2, M-6, M-8, X-6:
- M-2 / M-6: URL {{secret:NAME}} template stored without leaking the secret
  literal into trajectory or any harness log artifact under /logs/agent/.
- M-8: tool__updateServer with reenterSecrets prompts a second time and
  receives a distinct synthetic value.
- X-6: MCP secret store is separate from shell storeAs env ($E2E_LIFECYCLE_KEY
  is unset in workspace__runShell)."""

import json
import re
from pathlib import Path

from verifier_lib import EVIDENCE, load_evidence, load_trajectory, trajectory_steps

SEPARATE_FILE = Path("/app/store_separate.txt")
LOGS_DIR = Path("/logs/agent")
TEMPLATE = "{{secret:E2E_LIFECYCLE_KEY}}"
STORAGE_CALL = re.compile(r"tool__registerServer|tool__updateServer")


def _storage_call_blobs(steps: list[dict]) -> list[str]:
    """JSON of every tool__registerServer/tool__updateServer call's arguments,
    plus the text of the observations those calls produced.

    Scoping the template check here keeps the assertion non-vacuous: the
    template string also appears in the task instruction, so a whole-trajectory
    match would hold even if the server was stored with a literal secret.
    """
    blobs: list[str] = []
    call_ids: set[str] = set()
    call_step_ids: set[int] = set()
    for step in steps:
        step_has_call = False
        for call in step.get("tool_calls") or []:
            if not STORAGE_CALL.search(str(call.get("function_name") or "")):
                continue
            blobs.append(json.dumps(call.get("arguments")))
            call_id = call.get("tool_call_id")
            if call_id:
                call_ids.add(call_id)
            step_has_call = True
        if step_has_call:
            call_step_ids.add(step.get("step_id"))
    for step in steps:
        for result in (step.get("observation") or {}).get("results") or []:
            content = result.get("content")
            if not isinstance(content, str):
                continue
            source = result.get("source_call_id")
            if source in call_ids or (source is None and step.get("step_id") in call_step_ids):
                blobs.append(content)
    return blobs


def _delivered_answers(evidence: dict) -> list[dict]:
    return [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]


def test_initial_register_and_reenter_secrets_both_prompted():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = _delivered_answers(evidence)
    assert len(answers) >= 2, (
        "expected >=2 delivered secret prompts (register + reenterSecrets update), "
        f"got {answers!r}"
    )
    vals = [str(a.get("injectedValue") or "") for a in answers]
    assert all(vals), f"every prompt must record injectedValue: {answers!r}"
    assert vals[0] != vals[1], (
        f"reenterSecrets update must receive a distinct value, got {vals!r}"
    )


def test_template_recorded_and_stores_are_separate():
    blobs = _storage_call_blobs(trajectory_steps(load_trajectory()))
    assert blobs, (
        "expected at least one tool__registerServer/tool__updateServer call in "
        "the trajectory"
    )
    assert any(TEMPLATE in blob for blob in blobs), (
        f"missing {TEMPLATE} in the arguments or results of "
        "tool__registerServer/tool__updateServer — the server was not stored "
        "with the secret template"
    )
    assert SEPARATE_FILE.is_file(), "store_separate.txt missing"
    assert SEPARATE_FILE.read_text(encoding="utf-8").strip() == "SEPARATE"


def test_no_secret_literal_leaked_in_logs_or_trajectory():
    answers = _delivered_answers(load_evidence())
    secrets = [str(a["injectedValue"]).encode("utf-8") for a in answers if a.get("injectedValue")]
    assert secrets, "expected at least one recorded secret for leak scan"

    assert LOGS_DIR.is_dir(), f"log directory {LOGS_DIR} missing — leak scan would be skipped"
    for artifact in LOGS_DIR.rglob("*"):
        if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
            continue
        raw = artifact.read_bytes()
        for secret in secrets:
            assert secret not in raw, (
                f"synthetic MCP secret leaked into log artifact {artifact}"
            )
