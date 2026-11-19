"""2565 P-6 (prompt half): terminating a child session while it is waiting on
ui__askUser removes the pending prompt cleanly and lets the parent finish."""

from pathlib import Path

from verifier_lib import load_evidence

RESULT_FILE = Path("/app/result.txt")


def test_evidence_records_terminate_while_prompt_waiting():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    primary = evidence["sessionId"]
    term_records = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "terminateOnPrompt"
    ]
    assert term_records, (
        "harness never observed a waiting child prompt to terminate; "
        f"resolvedInteractions={evidence.get('resolvedInteractions')!r}"
    )
    rec = term_records[0]
    assert rec.get("sessionId") != primary, "expected child session termination"
    assert rec.get("requestId"), "terminateOnPrompt must record the waiting requestId"
    assert rec.get("terminated") is True, f"terminate failed: {rec!r}"
    assert rec.get("promptCleared") is True, (
        f"pendingUserPrompt was still present after termination: {rec!r}"
    )


def test_parent_completed_cleanly():
    assert RESULT_FILE.is_file(), "result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "TERMINATED_OK"
