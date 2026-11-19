"""2565 X-1: cancelling/stopping during a SUDO_PASSWORD prompt clears the
prompt and leaves the persistent shell clean and usable afterwards."""

from pathlib import Path

from verifier_lib import load_evidence

AFTER_FILE = Path("/app/after_cancel.txt")


def test_sudo_prompt_cancelled_and_shell_usable_after():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    cancels = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "cancel"
        and a.get("delivered") is True
    ]
    assert cancels, f"expected delivered cancel record, got {evidence!r}"
    assert AFTER_FILE.is_file(), "after_cancel.txt missing"
    assert AFTER_FILE.read_text(encoding="utf-8").strip() == "AFTER_CANCEL_OK"
