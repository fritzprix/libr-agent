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

# The value the harness injects as the storeAs answer. The README of the
# dataset exports LIBRAGENT_E2E_SYNTHETIC_SECRET with this exact value.
SECRET = "synthetic-e2e-fixture-only"
STORED = Path("/app/stored_env.txt")


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
    trajectory_blob = json.dumps(load_trajectory())
    assert SECRET not in trajectory_blob, (
        "the synthetic secret leaked into the session trajectory — storeAs "
        "values must be masked in logs, tool results, and the assistant context"
    )


def test_value_persisted_across_commands_without_being_printed():
    assert STORED.is_file(), (
        "the agent never proved the stored value survived into a later command "
        "(stored_env.txt missing)"
    )
    assert STORED.read_text(encoding="utf-8").strip() == "SECRET_PRESENT"
