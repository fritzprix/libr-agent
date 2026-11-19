"""#2901 P-9: after a ``storeAs`` answer, the synthetic secret must be absent
from every artifact the harness writes under ``/logs/agent/`` — the session
trajectory, the compaction summary, and the instance log — not just the
trajectory. The harness evidence record (``session_e2e_evidence.json``) is the
single sanctioned place where the exact injected value is kept, so it is the
one file excluded from the leak scan."""

import json
from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

STORED = Path("/app/p9_env.txt")
AGENT_LOGS = Path("/logs/agent")
EVIDENCE_NAME = "session_e2e_evidence.json"


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
    assert any("P9MARK_75" in text for text in texts), (
        "the storeAs command never observed P9MARK_75, so the prompt "
        "answer did not flow into the shell command"
    )


def test_value_persisted_across_commands_without_being_printed():
    assert STORED.is_file(), (
        "the agent never proved the stored value survived into a later command "
        "(p9_env.txt missing)"
    )
    assert STORED.read_text(encoding="utf-8").strip() == "P9_PRESENT"


def test_secret_never_leaked_into_agent_logs():
    recorded = _injected_secret(load_evidence())
    assert recorded, (
        "the harness did not record injectedValue for the answered prompt; "
        "without the exact injected value this leak check would be vacuous"
    )
    assert AGENT_LOGS.is_dir(), "harness log directory /logs/agent is not mounted"
    needle = recorded.encode("utf-8")
    offenders = []
    for path in sorted(AGENT_LOGS.rglob("*")):
        if not path.is_file() or path.name == EVIDENCE_NAME:
            continue
        if needle in path.read_bytes():
            offenders.append(str(path))
    assert not offenders, (
        "the synthetic storeAs secret leaked into: "
        + ", ".join(offenders)
        + " — storeAs values must be masked in the trajectory, the compaction "
        "summary, and the instance log; only the harness evidence record may "
        "hold the exact value"
    )
