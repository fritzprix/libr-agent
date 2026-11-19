"""#2672 K-1: a vague planning request triggers the clarify flow — the agent
asks questions (delivered through the E2E control plane) and keeps one
requirements document updated in the workspace. Graded from harness evidence,
tool results, and workspace files — never assistant prose."""

from pathlib import Path

from verifier_lib import load_evidence, load_trajectory, observation_result_texts, trajectory_steps

REQUIREMENTS = Path("/app/requirements.md")


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_agent_asked_clarifying_questions():
    evidence = load_evidence()
    answered = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]
    assert answered, (
        "the agent never asked anything — the clarify flow was not triggered. "
        f"Actions: {evidence.get('resolvedInteractions')!r}"
    )
    assert any(a.get("delivered") for a in answered), (
        f"no clarification answer was ever delivered: {answered!r}"
    )


def test_clarify_skill_was_loaded():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("clarify" in text.lower() for text in texts), (
        "no tool result mentions the clarify skill — it was never loaded"
    )


def test_requirements_document_maintained():
    assert REQUIREMENTS.is_file(), "the agent never kept a requirements.md"
    body = REQUIREMENTS.read_text(encoding="utf-8").strip()
    assert body, "requirements.md is empty"
    assert "#" in body, "requirements.md has no structure (no markdown headers)"
