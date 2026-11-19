"""2565 M-2, M-6, M-8, X-6:
- M-2 / M-6: URL {{secret:NAME}} template stored without leaking the secret
  literal into trajectory or any harness log artifact under /logs/agent/.
- M-8: tool__updateServer with reenterSecrets prompts a second time and
  receives a distinct synthetic value.
- X-6: MCP secret store is separate from shell storeAs env ($E2E_LIFECYCLE_KEY
  is unset in workspace__runShell)."""

import json
from pathlib import Path

from verifier_lib import EVIDENCE, load_evidence, load_trajectory

SEPARATE_FILE = Path("/app/store_separate.txt")
LOGS_DIR = Path("/logs/agent")
TEMPLATE = "{{secret:E2E_LIFECYCLE_KEY}}"


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
    traj_blob = json.dumps(load_trajectory())
    assert TEMPLATE in traj_blob, f"missing {TEMPLATE} in trajectory"
    assert SEPARATE_FILE.is_file(), "store_separate.txt missing"
    assert SEPARATE_FILE.read_text(encoding="utf-8").strip() == "SEPARATE"


def test_no_secret_literal_leaked_in_logs_or_trajectory():
    answers = _delivered_answers(load_evidence())
    secrets = [str(a["injectedValue"]).encode("utf-8") for a in answers if a.get("injectedValue")]
    assert secrets, "expected at least one recorded secret for leak scan"

    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            raw = artifact.read_bytes()
            for secret in secrets:
                assert secret not in raw, (
                    f"synthetic MCP secret leaked into log artifact {artifact}"
                )
