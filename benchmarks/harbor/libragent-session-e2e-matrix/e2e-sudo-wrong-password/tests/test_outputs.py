"""2565 B-2: wrong SUDO_PASSWORD returns a clean failure without hanging and
never leaks the password literal into trajectory or logs."""

import json
from pathlib import Path

from verifier_lib import (
    EVIDENCE,
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

RESULT_FILE = Path("/app/result.txt")
LOGS_DIR = Path("/logs/agent")


def test_wrong_sudo_password_handled_without_hang_or_leak():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]
    assert answers, f"expected delivered SUDO_PASSWORD prompt, got {evidence!r}"
    secret = str(answers[0].get("injectedValue") or "")
    assert secret, "missing injectedValue"

    assert RESULT_FILE.is_file(), "result.txt missing (session may have hung)"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "WRONG_PASSWORD_HANDLED"

    # S1.1: the delivered answer alone is not the evidence - the sudo tool
    # result itself must report the rejection, not just the agent saying so.
    trajectory = load_trajectory()
    obs = "\n".join(observation_result_texts(trajectory_steps(trajectory))).lower()
    assert any(
        marker in obs
        for marker in ("incorrect password", "is wrong", "try again", "sorry, try")
    ), f"no sudo password-rejection observed in tool results: {obs[:400]!r}"

    assert secret not in json.dumps(trajectory), "wrong password leaked in trajectory"
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"wrong password leaked into {artifact}"
            )
