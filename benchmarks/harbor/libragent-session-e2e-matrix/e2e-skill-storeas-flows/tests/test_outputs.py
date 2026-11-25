"""2565 S-1, S-2, S-5, S-6 & 2685 E-3: verifies the generic storeAs secret
collection mechanism under the four skills' variable names
(TELEGRAM_API_HASH, X_PASSWORD, LIBRAGENT_EMAIL_PASSWORD,
LIBRAGENT_GOOGLE_CLIENT_SECRET): each persistent shell env receives its
distinct synthetic value and no injected secret literal leaks into any
artifact under /logs/agent/. The bundled skills' own instructions are NOT
exercised — the harness drives the workspace__runInPersistentShell calls
directly, so this proves the storeAs mechanism, not the skills' text."""

import hashlib
from pathlib import Path

from verifier_lib import EVIDENCE, load_evidence

HASH_FILES = [
    Path("/app/telegram.sha256"),
    Path("/app/x.sha256"),
    Path("/app/email.sha256"),
    Path("/app/calendar.sha256"),
]
LOGS_DIR = Path("/logs/agent")


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


def test_all_four_skill_secrets_delivered_and_matched_by_hash():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = _delivered_answers(evidence)
    assert len(answers) >= 4, f"expected >=4 delivered answers, got {answers!r}"

    injected = [str(a.get("injectedValue") or "") for a in answers[:4]]
    assert all(injected), f"missing injectedValue in answers: {answers[:4]!r}"
    assert len(set(injected)) == 4, f"expected 4 distinct secrets, got {injected!r}"

    for path, expected_secret in zip(HASH_FILES, injected, strict=True):
        assert path.is_file(), f"missing hash proof file {path}"
        observed = path.read_text(encoding="utf-8").strip().lower()
        expected = _sha256_hex(expected_secret)
        assert observed == expected, (
            f"hash mismatch in {path}: got {observed!r}, expected {expected!r}"
        )


def test_no_secret_literal_leaked_in_logs():
    answers = _delivered_answers(load_evidence())
    secrets = [str(a["injectedValue"]).encode("utf-8") for a in answers if a.get("injectedValue")]
    assert secrets, "expected recorded secrets for leak scan"

    assert LOGS_DIR.is_dir(), f"log directory {LOGS_DIR} missing — leak scan would be skipped"
    for artifact in LOGS_DIR.rglob("*"):
        if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
            continue
        raw = artifact.read_bytes()
        for secret in secrets:
            assert secret not in raw, (
                f"synthetic skill secret leaked into log artifact {artifact}"
            )
