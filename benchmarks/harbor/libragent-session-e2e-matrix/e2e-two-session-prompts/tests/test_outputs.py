"""2565 X-2 (smoke test only): a parent session spawns a companion session
and both hold waiting storeAs prompts; the harness answers each one. Graded
from harness evidence (per-session control records), the primary trajectory,
and workspace files — never assistant prose. This proves the harness plumbing
works (each session's prompt was answered and delivered); it does NOT prove
product answer-channel isolation — the ``-child`` suffix is appended by the
harness itself and nothing here shows the two prompts were pending at once.
Real X-2 coverage is follow-up work under #2901. The child's own container is
not visible here; its marker arrives through the parent's
``agent__checkSession`` round trip."""

from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

CHILD_RESULT = Path("/app/child_result.txt")
PRIMARY_ENV_PROOF = Path("/app/a_env.txt")


def _answers(evidence: dict) -> list[dict]:
    return [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_both_sessions_prompts_answered():
    evidence = load_evidence()
    primary = evidence.get("sessionId")
    answers = _answers(evidence)
    session_ids = {a.get("sessionId") for a in answers}
    assert primary in session_ids, (
        "the primary session's waiting prompt was never answered; "
        f"records: {answers!r}"
    )
    child_ids = session_ids - {primary}
    assert len(child_ids) == 1, (
        f"expected exactly one companion session with an answered prompt, "
        f"got {child_ids!r}"
    )
    for sid in (primary, next(iter(child_ids))):
        assert any(
            a.get("sessionId") == sid and a.get("delivered") for a in answers
        ), f"no delivered answer for session {sid}: {answers!r}"


def test_harness_delivered_a_separate_answer_to_each_session():
    evidence = load_evidence()
    primary = evidence.get("sessionId")
    answers = _answers(evidence)
    primary_vals = {
        a.get("injectedValue") for a in answers if a.get("sessionId") == primary
    }
    child_vals = {
        a.get("injectedValue")
        for a in answers
        if a.get("sessionId") not in (primary, None)
    }
    assert all(primary_vals) and all(child_vals), (
        "every answered prompt must record its injectedValue: "
        f"primary={primary_vals!r} child={child_vals!r}"
    )
    # Smoke check on the harness itself: the -child suffix is appended by the
    # harness (automation.py), so this validates the evidence plumbing, not
    # product answer-channel isolation.
    assert all(str(v).endswith("-child") for v in child_vals)
    assert all(not str(v).endswith("-child") for v in primary_vals)
    assert primary_vals.isdisjoint(child_vals), (
        "the harness recorded the same injected answer value for both sessions"
    )


def test_primary_answer_reached_its_own_shell():
    assert PRIMARY_ENV_PROOF.is_file(), (
        "the agent never proved its own storeAs value survived into a later "
        "command (a_env.txt missing)"
    )
    assert PRIMARY_ENV_PROOF.read_text(encoding="utf-8").strip() == "A_PRESENT"


def test_primary_observed_its_own_marker():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("AMARK_67" in text for text in texts), (
        "the primary session never observed AMARK_67, so its answered "
        "command never ran to completion"
    )


def test_child_result_carries_its_own_marker():
    assert CHILD_RESULT.is_file(), (
        "the parent never wrote the child session's result (child_result.txt "
        "missing) — the checkSession round trip did not complete"
    )
    assert "BMARK_42" in CHILD_RESULT.read_text(encoding="utf-8"), (
        "the child session's result does not carry BMARK_42 — the child "
        "never observed its own answered marker"
    )
