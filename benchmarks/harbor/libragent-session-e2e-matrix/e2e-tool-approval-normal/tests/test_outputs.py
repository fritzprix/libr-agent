"""#2901 (2672 R-2, `normal` half): in normal mode a workspace write opens a
tool-approval request, the harness answers it through the E2E control plane,
and the approved tool call actually executes. Graded from harness evidence,
tool results, and workspace files — never assistant prose."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

MARKER = Path("/app/approval_marker.txt")


def test_evidence_present_and_control_plane_enabled():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"
    assert evidence.get("controlPlaneConfigured"), (
        "the trial did not run against a debug binary with the E2E control "
        "plane enabled (see run logs for the control-plane warning)"
    )


def test_standard_tool_approval_was_granted_via_control_plane():
    evidence = load_evidence()
    approved = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "toolApproval" and action.get("approved") is True
    ]
    assert approved, (
        "no tool approval was answered through the control plane; normal mode "
        "must open one for workspace__writeFile. Actions: "
        f"{evidence.get('resolvedInteractions')!r}"
    )


def test_approved_tool_call_produced_a_real_result():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any(
        "approval_marker.txt" in text and "error" not in text.lower()
        for text in texts
    ), f"no successful write tool result found: {texts[:3]!r}"


def test_marker_written_after_approval():
    assert MARKER.is_file(), "the approved write never reached the workspace"
    assert MARKER.read_text(encoding="utf-8").strip() == "APPROVED"


def test_no_approval_pending_at_completion():
    evidence = load_evidence()
    interactions = evidence.get("interactions")
    if isinstance(interactions, dict):
        assert not interactions.get("pendingApprovals"), (
            "approvals were still pending at completion: "
            f"{interactions['pendingApprovals']!r}"
        )
