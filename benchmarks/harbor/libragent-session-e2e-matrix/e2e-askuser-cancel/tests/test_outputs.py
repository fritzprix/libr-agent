"""2672 C-4 / 2565 A-3: cancelling a ui__askUser dialog must return an
informational result and let the session continue without an error state.
The harness cancels the waiting prompt through the E2E control plane (the
same path the dialog's Cancel button drives), standing in for the GUI
Cancel click and the Stop button of 2565 P-3. Graded from harness evidence,
tool results, and workspace files — never assistant prose."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

CANCELLED = Path("/app/cancel.txt")


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_prompt_was_cancelled_and_delivered():
    evidence = load_evidence()
    cancelled = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "cancel"
    ]
    assert cancelled, (
        "the harness never cancelled the waiting prompt; the agent may be "
        f"stuck or answered instead. Actions: {evidence.get('resolvedInteractions')!r}"
    )
    assert cancelled[0].get("delivered"), (
        f"the cancel never reached the session: {cancelled[0]!r}"
    )


def test_cancel_reached_the_agent_as_a_tool_result():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    # USER_PROMPT_CANCELLED_MESSAGE in src-tauri/src/agent/state/user_prompt.rs —
    # match the exact informational result, not any result that mentions the word.
    assert any("User cancelled the prompt." in text for text in texts), (
        "no tool result carries the informational cancel result "
        "(USER_PROMPT_CANCELLED_MESSAGE) — the cancel never reached the agent "
        "as a tool result"
    )


def test_session_continued_after_cancel():
    assert CANCELLED.is_file(), (
        "the agent did not continue to completion after the cancel "
        "(cancel.txt missing — it likely treated the cancel as a failure)"
    )
    assert CANCELLED.read_text(encoding="utf-8").strip() == "CANCELLED_OK"
