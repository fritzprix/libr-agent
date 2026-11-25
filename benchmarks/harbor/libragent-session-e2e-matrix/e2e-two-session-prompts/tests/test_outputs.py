"""2565 X-2: a parent session spawns a companion session so both hold waiting
storeAs prompts simultaneously. The harness holds prompt resolution until both
sessions have a pendingUserPrompt in the same poll cycle, answers the child
first with a distinct synthetic value, and answers the parent second.

Graded from harness evidence, the primary trajectory, and workspace files —
never assistant prose. Each session proves which exact value it received by
writing the SHA-256 hex digest of its env var (never printing the literal
secret)."""

import hashlib
import json
from pathlib import Path

from verifier_lib import (
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

CHILD_RESULT = Path("/app/child_result.txt")
PRIMARY_HASH_PROOF = Path("/app/a_hash.txt")


def _answers(evidence: dict) -> list[dict]:
    return [
        action
        for action in evidence.get("resolvedInteractions") or []
        if action.get("kind") == "userPrompt" and action.get("action") == "answer"
    ]


def _sha256_hex(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def test_evidence_present():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "harness must write session_e2e_evidence.json"


def test_both_sessions_prompts_overlapped_and_child_answered_first():
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
    child_id = next(iter(child_ids))

    child_idx = next(
        i
        for i, a in enumerate(answers)
        if a.get("sessionId") == child_id and a.get("delivered") is True
    )
    primary_idx = next(
        i
        for i, a in enumerate(answers)
        if a.get("sessionId") == primary and a.get("delivered") is True
    )
    assert child_idx < primary_idx, (
        "session B (child) must be answered before session A (primary): "
        f"child_idx={child_idx}, primary_idx={primary_idx}"
    )
    assert answers[child_idx].get("simultaneousPending") is True, (
        "child prompt answer must be recorded from an overlapping pending poll cycle"
    )
    assert answers[primary_idx].get("simultaneousPending") is True, (
        "primary prompt answer must be recorded from an overlapping pending poll cycle"
    )


def test_each_session_received_its_own_distinct_answer_without_leaking():
    evidence = load_evidence()
    primary = evidence.get("sessionId")
    answers = _answers(evidence)

    primary_ans = next(
        a for a in answers if a.get("sessionId") == primary and a.get("delivered") is True
    )
    child_ans = next(
        a
        for a in answers
        if a.get("sessionId") not in (primary, None) and a.get("delivered") is True
    )
    primary_val = str(primary_ans.get("injectedValue") or "")
    child_val = str(child_ans.get("injectedValue") or "")
    assert primary_val and child_val, "both sessions must record injectedValue"
    assert primary_val != child_val, "parent and child must receive distinct values"

    assert PRIMARY_HASH_PROOF.is_file(), "a_hash.txt missing"
    primary_hash = PRIMARY_HASH_PROOF.read_text(encoding="utf-8").strip().lower()
    assert primary_hash == _sha256_hex(primary_val), (
        f"primary session env hash mismatch: got {primary_hash!r}, "
        f"expected sha256(primary_val)={_sha256_hex(primary_val)!r}"
    )

    assert CHILD_RESULT.is_file(), "child_result.txt missing"
    child_text = CHILD_RESULT.read_text(encoding="utf-8")
    assert "BMARK_42" in child_text, "child_result.txt missing BMARK_42"
    assert _sha256_hex(child_val) in child_text.lower(), (
        "child_result.txt missing sha256(child_val) — child did not receive its own answer"
    )

    # Neither literal secret may leak into workspace files or trajectory.
    traj_blob = json.dumps(load_trajectory())
    for literal in (primary_val, child_val):
        assert literal not in traj_blob, "secret literal leaked into trajectory"
        assert literal not in child_text, "secret literal leaked into child_result.txt"
        assert literal not in primary_hash, "secret literal leaked into a_hash.txt"


def test_primary_observed_its_own_marker():
    texts = observation_result_texts(trajectory_steps(load_trajectory()))
    assert any("AMARK_67" in text for text in texts), (
        "the primary session never observed AMARK_67, so its answered "
        "command never ran to completion"
    )
