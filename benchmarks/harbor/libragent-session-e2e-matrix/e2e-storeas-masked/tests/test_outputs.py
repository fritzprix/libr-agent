"""#2901 B-12: a ``storeAs`` prompt answer (the synthetic secret) is injected
into the shell session as an env var without the secret literal ever
appearing in the trajectory or tool results. Graded from harness evidence,
tool results, and workspace files — never assistant prose."""

import json
from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

STORED = Path("/app/stored_env.txt")


def _injected_secret(evidence: dict) -> str | None:
    """The exact (synthetic) value the harness injected as the storeAs answer.

    The host harness reads ``LIBRAGENT_E2E_SYNTHETIC_SECRET`` and records the
    value it sent in ``session_e2e_evidence.json`` as ``injectedValue``; this
    verifier container cannot see that env var, so the evidence record — not
    an env guess — is what keeps the leak check non-vacuous when CI sets a
    different secret (gatekeeper G3.1).
    """
    for action in evidence.get("resolvedInteractions") or []:
        if action.get("kind") == "userPrompt" and action.get("action") == "answer":
            value = action.get("injectedValue")
            if value:
                return str(value)
    return None


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_storeas_prompt_was_answered_and_delivered():
    evidence = load_evidence()
    answered = [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]
    assert answered, (
        "the harness never answered the storeAs user prompt; the agent is "
        f"stuck waiting. Actions: {evidence.get('resolvedInteractions')!r}"
    )
    assert answered[0].get("delivered"), (
        "the storeAs answer was never delivered to the session: "
        f"{answered[0]!r}"
    )


def test_secret_value_reached_the_shell():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("E2E_MARKER_102" in text for text in texts), (
        "the storeAs command never observed E2E_MARKER_102, so the prompt "
        "answer did not flow into the shell command"
    )


def test_secret_never_leaked_into_trajectory():
    recorded = _injected_secret(load_evidence())
    assert recorded, (
        "the harness did not record injectedValue for the answered prompt; "
        "without the exact injected value this leak check would be vacuous"
    )
    trajectory_blob = json.dumps(load_trajectory())
    assert recorded not in trajectory_blob, (
        "the synthetic secret leaked into the session trajectory — storeAs "
        "values must be masked in logs, tool results, and the assistant context"
    )


def test_value_persisted_across_commands_without_being_printed():
    assert STORED.is_file(), (
        "the agent never proved the stored value survived into a later command "
        "(stored_env.txt missing)"
    )
    assert STORED.read_text(encoding="utf-8").strip() == "SECRET_PRESENT"
