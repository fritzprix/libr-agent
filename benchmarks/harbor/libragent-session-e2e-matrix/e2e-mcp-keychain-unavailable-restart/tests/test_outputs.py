"""2672 M-5: keychain unavailable -> encrypted_secrets fallback persists across restart."""

import json
from pathlib import Path

from verifier_lib import (
    EVIDENCE,
    injected_values,
    load_evidence,
    load_trajectory,
    require_session_evidence,
)

RESULT_FILE = Path("/app/m5_result.txt")
MOCK_LOG = Path("/logs/agent/e2e_mock_keychain.log")
RESTART_EVIDENCE = Path("/logs/agent/session_e2e_restart_evidence.json")
LOGS_DIR = Path("/logs/agent")


def test_keychain_unavailable_encrypted_fallback_survives_restart():
    require_session_evidence()
    evidence = load_evidence()
    secrets = injected_values(evidence)
    assert secrets, f"expected delivered secret prompt, got {evidence!r}"

    assert MOCK_LOG.is_file(), (
        "e2e_mock_keychain.log missing — trial did not run with LIBRAGENT_E2E_MOCK_KEYCHAIN=unavailable"
    )
    entries = [
        json.loads(line)
        for line in MOCK_LOG.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert any(
        e.get("op") == "available" and e.get("outcome") == "false" and e.get("mode") == "unavailable"
        for e in entries
    ), f"expected mock keychain available=false (unavailable), got {entries!r}"

    assert RESTART_EVIDENCE.is_file(), "session_e2e_restart_evidence.json missing"
    restart = json.loads(RESTART_EVIDENCE.read_text(encoding="utf-8"))
    assert restart.get("restartExecuted") is True, f"instance restart failed: {restart!r}"

    http_by_name = {p["name"]: p for p in restart.get("http") or [] if isinstance(p, dict)}
    assert http_by_name.get("health", {}).get("status") == 200, (
        f"post-restart /health failed: {http_by_name!r}"
    )

    sql_by_name = {p["name"]: p for p in restart.get("sql") or [] if isinstance(p, dict)}
    rows = (sql_by_name.get("mcp_server") or {}).get("rows") or []
    assert len(rows) == 1, f"expected persisted e2e-m5-fallback row after restart, got {rows!r}"
    name, config, _secret_names, has_encrypted = rows[0]
    assert name == "e2e-m5-fallback"
    assert "{{secret:E2E_M5_KEY}}" in str(config), (
        f"expected template placeholder in config, got {config!r}"
    )
    assert has_encrypted == 1, "expected non-empty encrypted_secrets fallback column after restart"

    assert RESULT_FILE.is_file(), "m5_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "M5_FALLBACK_DONE"


def test_secret_literal_never_leaks():
    secrets = injected_values(load_evidence())
    if not secrets:
        return
    secret = secrets[0]
    assert secret not in json.dumps(load_trajectory()), "secret leaked into trajectory"
    if RESTART_EVIDENCE.is_file():
        assert secret not in RESTART_EVIDENCE.read_text(encoding="utf-8"), (
            "secret literal leaked into post-restart SQL/HTTP evidence"
        )
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"secret literal leaked into {artifact}"
            )
