"""2565 P-6 (Windows process half): the child session's persistent PowerShell
shell is alive while the child waits on ui__askUser, and after the harness
terminates the child the recorded shell PID is gone and stays gone through a
settle window.

Scope note: this task proves "alive while waiting, killed by terminate, same
PID stays gone". It cannot see a replacement shell (a respawn would carry a
new PID); the no-replacement guarantee is owned by the backend unit test
``force_kills_persistent_shell_without_leaving_manager_entry``
(``workspace_server/tests.rs``) over ``force_kill_shell``.

The harness (automation.py ``check_persistent_shell_after_terminate``) reads
``p6_shell_pid.txt`` from the shared workspace — the child is spawned with
``workspaceOverride: "current"`` so it lands in this task's workspace, which
is mounted here at /app. Probe failures surface as null flags (the verifier
requires strict True), so a failed probe can never pass."""

from pathlib import Path

from verifier_lib import WORKSPACE, load_evidence

RESULT_FILE = Path("/app/result.txt")
PID_FILE = WORKSPACE / "p6_shell_pid.txt"


def _terminate_record(evidence: dict) -> dict:
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
    assert rec.get("sessionId") != evidence["sessionId"], (
        "expected child session termination"
    )
    assert rec.get("requestId"), "terminateOnPrompt must record the waiting requestId"
    assert rec.get("terminated") is True, f"terminate failed: {rec!r}"
    assert rec.get("promptCleared") is True, (
        f"pendingUserPrompt was still present after termination: {rec!r}"
    )
    return rec


def test_evidence_records_shell_lifecycle_across_terminate():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    rec = _terminate_record(evidence)
    assert isinstance(rec.get("shellPid"), int) and rec["shellPid"] > 0, (
        f"persistent shell PID not recorded (agent skipped step 1?): {rec!r}"
    )
    assert rec.get("shellAliveWhileWaiting") is True, (
        "persistent shell was NOT alive while the child prompt waited: "
        f"{rec!r}"
    )
    assert rec.get("shellGoneAfterTerminate") is True, (
        f"persistent shell survived session termination: {rec!r}"
    )
    assert rec.get("shellStillGoneAfterSettle") is True, (
        f"recorded shell PID was alive again after the settle window: {rec!r}"
    )


def test_pid_file_in_workspace_matches_evidence():
    """Cross-check the evidence against the workspace artifact: the persistent
    shell really wrote its PID into the shared workspace, and the harness read
    exactly that value (not a fabricated one)."""
    evidence = load_evidence()
    rec = _terminate_record(evidence)
    assert PID_FILE.is_file(), (
        "p6_shell_pid.txt missing: the child never ran its persistent shell "
        "in the shared workspace"
    )
    recorded = PID_FILE.read_text(encoding="ascii").strip()
    assert recorded.isdigit(), f"p6_shell_pid.txt is not a pid: {recorded!r}"
    assert int(recorded) == rec.get("shellPid"), (
        f"evidence shellPid {rec.get('shellPid')!r} does not match workspace "
        f"p6_shell_pid.txt {recorded!r}"
    )


def test_parent_completed_cleanly():
    assert RESULT_FILE.is_file(), "result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "SHELL_TREE_CLEAN"
