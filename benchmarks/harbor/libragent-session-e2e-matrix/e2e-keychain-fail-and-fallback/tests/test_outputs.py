"""2565 M-7 / 2672 M-4 & 2672 M-5 (unavailable keychain half): verifies MCP
secret registration runs unattended against the mock/fallback keychain backend
with zero host keychain prompts and zero secret literal leaks."""

import json
from pathlib import Path

from verifier_lib import EVIDENCE, load_evidence, load_trajectory

RESULT_FILE = Path("/app/keychain_result.txt")
LOGS_DIR = Path("/logs/agent")


def test_keychain_secret_flow_completed_without_leak():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]
    assert answers, f"expected delivered secret prompt, got {evidence!r}"
    secret = str(answers[0].get("injectedValue") or "")
    assert secret, "missing injectedValue"

    assert RESULT_FILE.is_file(), "keychain_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "KEYCHAIN_FLOW_DONE"

    traj_blob = json.dumps(load_trajectory())
    assert "{{secret:E2E_KEYCHAIN_SECRET}}" in traj_blob
    assert secret not in traj_blob, "secret literal leaked into trajectory"
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"secret literal leaked into {artifact}"
            )
