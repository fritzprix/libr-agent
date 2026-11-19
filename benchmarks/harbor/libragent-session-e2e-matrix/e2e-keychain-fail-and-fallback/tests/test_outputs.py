"""2565 M-7 / 2672 M-4 (fail-write half): the task runs the instance with
``LIBRAGENT_E2E_MOCK_KEYCHAIN=fail-write``; the harness copies the mock's
invocation log and store file into /logs/agent, so this verifier observes the
mock itself - not just the agent's narration (G1.4).

Proved here: the secret write was attempted against the mock and failed
(M-4), and no secret ever persisted to the keychain backend (M-7 "row
unchanged", read independently of the agent through the store copy).
M-5 (unavailable -> file fallback) is a different mock mode and stays
deferred in the manifest until a task runs that mode."""

import json
from pathlib import Path

from verifier_lib import (
    EVIDENCE,
    load_evidence,
    load_trajectory,
    observation_result_texts,
    trajectory_steps,
)

RESULT_FILE = Path("/app/keychain_result.txt")
MOCK_LOG = Path("/logs/agent/e2e_mock_keychain.log")
MOCK_STORE = Path("/logs/agent/e2e_mock_keychain.json")
LOGS_DIR = Path("/logs/agent")


def _mock_log_entries() -> list[dict]:
    assert MOCK_LOG.is_file(), (
        "mock keychain log missing from /logs/agent - the trial did not run "
        "with LIBRAGENT_E2E_MOCK_KEYCHAIN=fail-write, so the mock was never observed"
    )
    entries = [
        json.loads(line)
        for line in MOCK_LOG.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert entries, "mock keychain log is empty - no secret operation reached the mock"
    return entries


def test_secret_write_attempted_and_failed_against_mock():
    evidence = load_evidence()
    assert evidence.get("sessionId"), "missing sessionId in E2E evidence"
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]
    assert answers, f"expected delivered secret prompt, got {evidence!r}"

    entries = _mock_log_entries()
    set_errors = [
        e
        for e in entries
        if e.get("op") == "set" and e.get("outcome") == "error" and e.get("mode") == "fail-write"
    ]
    assert set_errors, (
        f"no fail-write set error in mock log: {entries!r}"
    )
    available = [e for e in entries if e.get("op") == "available"]
    assert any(e.get("outcome") == "true" for e in available), (
        "fail-write must keep the keychain available: "
        f"{available!r}"
    )

    # M-7 independent read: the store copy is written by the app process,
    # not the agent. A failed write must leave no persisted entry.
    if MOCK_STORE.is_file():
        store = json.loads(MOCK_STORE.read_text(encoding="utf-8") or "{}")
        assert store == {}, f"secret persisted despite fail-write: {list(store)!r}"

    # The registration error must be visible in the tool results, not only
    # in the agent's final summary.
    trajectory = load_trajectory()
    obs = "\n".join(observation_result_texts(trajectory_steps(trajectory))).lower()
    assert "mock keychain write failed" in obs, (
        f"fail-write error not observed in tool results: {obs[:400]!r}"
    )

    assert RESULT_FILE.is_file(), "keychain_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "KEYCHAIN_FLOW_DONE"


def test_secret_literal_never_leaks():
    evidence = load_evidence()
    answers = [
        a
        for a in evidence.get("resolvedInteractions") or []
        if a.get("kind") == "userPrompt"
        and a.get("action") == "answer"
        and a.get("delivered") is True
    ]
    secret = str(answers[0].get("injectedValue") or "") if answers else ""
    if not secret:
        return

    for path in (MOCK_LOG, MOCK_STORE):
        if path.is_file():
            assert secret.encode("utf-8") not in path.read_bytes(), (
                f"secret literal leaked into {path.name}"
            )
    assert secret not in json.dumps(load_trajectory()), "secret literal leaked into trajectory"
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"secret literal leaked into {artifact}"
            )
