"""2565 P-1, 2565 B-1, 2672 S-2: runInPersistentShell storeAs SUDO_PASSWORD lets
plain sudo succeed via askpass, a follow-up plain sudo succeeds without
re-prompting, pkexec is never invoked, and the password never leaks.

Askpass usage is proven positively by the uid-0 artifacts: the persistent
shell has no tty and no DISPLAY, so plain `sudo` can only succeed if the
product's private askpass helper answered the prompt. The image's own
`e2e-askpass` shim is the bypass detector - the product never calls it, so
its log must exist and stay empty (G1.3)."""

import json
from pathlib import Path

from verifier_lib import EVIDENCE, load_evidence, load_trajectory, privileged_log

FIRST_FILE = Path("/app/sudo_first.txt")
UID_FILE = Path("/app/sudo_uid.txt")
LOGS_DIR = Path("/logs/agent")


def _delivered_answers(evidence: dict) -> list[dict]:
    return [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]


def test_sudo_password_prompted_once_and_sudo_succeeded():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = _delivered_answers(evidence)
    assert len(answers) == 1, f"expected exactly 1 SUDO_PASSWORD prompt, got {answers!r}"

    assert FIRST_FILE.is_file(), "sudo_first.txt missing"
    assert FIRST_FILE.read_text(encoding="utf-8").strip() == "SUDO_FIRST_OK"

    assert UID_FILE.is_file(), "sudo_uid.txt missing"
    assert UID_FILE.read_text(encoding="utf-8").strip() == "0"


def test_pkexec_and_image_askpass_never_used_and_password_never_leaked():
    # Existence is asserted, so "never invoked" is distinct from "unreachable".
    privileged_log("pkexec.log", expect_records=False)
    privileged_log("askpass.log", expect_records=False)

    answers = _delivered_answers(load_evidence())
    secret = str(answers[0].get("injectedValue") or "")
    assert secret, "missing injectedValue in evidence"
    traj_blob = json.dumps(load_trajectory())
    assert secret not in traj_blob, "SUDO_PASSWORD leaked into trajectory"
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"SUDO_PASSWORD leaked into {artifact}"
            )
