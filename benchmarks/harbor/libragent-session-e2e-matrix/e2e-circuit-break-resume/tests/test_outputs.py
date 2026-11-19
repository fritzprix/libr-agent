"""#2901: the harness trips the tool-loop circuit breaker and resumes it via
the debug E2E control plane (``POST /api/e2e-control/sessions/:ref/circuit-break-resume``),
which injects the same ``ui__resumeCircuitBreak`` tool call the GUI Resume
button drives. Graded from harness evidence, tool results, and workspace
files — never assistant prose."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

MARKER = Path("/app/break_marker.txt")


def test_evidence_present_and_control_plane_enabled():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"
    assert evidence.get("controlPlaneConfigured"), (
        "the trial did not run against a debug binary with the E2E control "
        "plane enabled (see run logs for the control-plane warning)"
    )


def test_circuit_break_was_resumed_via_control_plane():
    evidence = load_evidence()
    resumed = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "circuitBreakResume"
    ]
    assert resumed, (
        "the harness never resumed a circuit break; either the breaker was "
        f"never tripped or the resume endpoint failed. Actions: "
        f"{evidence.get('resolvedInteractions')!r}"
    )
    assert isinstance(resumed[0].get("repetitionCount"), int)


def test_resume_guidance_reached_the_agent():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("Execution Resumed" in text for text in texts), (
        "the ui__resumeCircuitBreak guidance never reached the session"
    )


def test_agent_completed_after_resume():
    assert MARKER.is_file(), "the agent never wrote break_marker.txt after resume"
    assert MARKER.read_text(encoding="utf-8").strip() == "RESUMED"


def test_no_circuit_break_pending_at_completion():
    evidence = load_evidence()
    interactions = evidence.get("interactions")
    if isinstance(interactions, dict):
        assert not interactions.get("pendingCircuitBreak"), (
            "a circuit break was still pending at completion: "
            f"{interactions['pendingCircuitBreak']!r}"
        )
