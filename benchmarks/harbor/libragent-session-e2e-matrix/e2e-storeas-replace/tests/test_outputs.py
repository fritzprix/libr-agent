"""2565 B-8: re-storing the same storeAs variable with a different value
replaces the old value in the persistent shell while keeping cwd and never
leaking either literal secret."""

import hashlib
import json
from pathlib import Path

from verifier_lib import load_evidence, load_trajectory

HASH1 = Path("/app/hash1.txt")
HASH2 = Path("/app/hash2.txt")
CWD1 = Path("/app/cwd1.txt")
CWD2 = Path("/app/cwd2.txt")


def _sha256_hex(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _delivered_answers(evidence: dict) -> list[dict]:
    return [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]


def test_evidence_has_two_distinct_answers():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = _delivered_answers(evidence)
    assert len(answers) >= 2, f"expected >=2 delivered answers, got {answers!r}"
    v1 = str(answers[0].get("injectedValue") or "")
    v2 = str(answers[1].get("injectedValue") or "")
    assert v1 and v2, f"missing injectedValue in answers: {answers!r}"
    assert v1 != v2, f"replacement test requires two distinct values, got {v1!r} twice"


def test_second_value_replaced_first_and_cwd_preserved():
    answers = _delivered_answers(load_evidence())
    v1 = str(answers[0]["injectedValue"])
    v2 = str(answers[1]["injectedValue"])

    assert HASH1.is_file() and HASH2.is_file(), "hash1.txt or hash2.txt missing"
    h1 = HASH1.read_text(encoding="utf-8").strip().lower()
    h2 = HASH2.read_text(encoding="utf-8").strip().lower()
    assert h1 == _sha256_hex(v1), f"hash1 mismatch: got {h1!r}, expected {_sha256_hex(v1)!r}"
    assert h2 == _sha256_hex(v2), f"hash2 mismatch: got {h2!r}, expected {_sha256_hex(v2)!r}"
    assert h1 != h2, "hashes must differ when value is replaced"

    assert CWD1.is_file() and CWD2.is_file(), "cwd1.txt or cwd2.txt missing"
    c1 = CWD1.read_text(encoding="utf-8").strip()
    c2 = CWD2.read_text(encoding="utf-8").strip()
    assert c1.endswith("/subdir"), f"expected cwd1 in /subdir, got {c1!r}"
    assert c2 == c1, f"cwd changed across storeAs replacement: {c1!r} -> {c2!r}"


def test_no_secret_literal_leaked():
    answers = _delivered_answers(load_evidence())
    traj_blob = json.dumps(load_trajectory())
    for ans in answers:
        val = str(ans.get("injectedValue") or "")
        if val:
            assert val not in traj_blob, "secret literal leaked into trajectory"
            for path in (HASH1, HASH2, CWD1, CWD2):
                if path.is_file():
                    assert val not in path.read_text(encoding="utf-8"), (
                        f"secret literal leaked into {path}"
                    )
