"""#2901: a ``ui__askUser`` dialog is answered unattended through the debug E2E
control plane, and the agent writes the delivered answer back to the
workspace. Graded from harness evidence, tool results, and workspace files —
never assistant prose."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

ANSWER_FILE = Path("/app/answer.txt")


def test_evidence_present_and_control_plane_enabled():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"
    assert evidence.get("controlPlaneConfigured"), (
        "the trial did not run against a debug binary with the E2E control "
        "plane enabled (see run logs for the control-plane warning)"
    )


def test_answer_was_delivered_via_control_plane():
    evidence = load_evidence()
    answered = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt"
        and action.get("action") == "answer"
        and action.get("delivered") is True
    ]
    assert answered, (
        "the harness never delivered an answer to a user prompt; the "
        "ui__askUser path was not exercised. Actions: "
        f"{evidence.get('resolvedInteractions')!r}"
    )


def test_written_answer_matches_a_real_tool_result():
    assert ANSWER_FILE.is_file(), "the agent never wrote answer.txt"
    body = ANSWER_FILE.read_text(encoding="utf-8").strip()
    assert body, "answer.txt is empty"
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any(body in text for text in texts), (
        "answer.txt content never appeared in any tool result — the agent "
        f"invented it instead of relaying the dialog answer: {body!r}"
    )


def test_no_prompt_pending_at_completion():
    evidence = load_evidence()
    interactions = evidence.get("interactions")
    if isinstance(interactions, dict):
        assert interactions.get("pendingUserPrompt") is None, (
            "a user prompt was still pending at completion: "
            f"{interactions['pendingUserPrompt']!r}"
        )
