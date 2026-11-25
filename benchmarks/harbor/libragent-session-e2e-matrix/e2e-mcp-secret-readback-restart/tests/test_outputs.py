"""2672 R-4: {{secret:NAME}} server stored, masked, and read back after restart."""

import json
from pathlib import Path

from verifier_lib import (
    EVIDENCE,
    injected_values,
    load_evidence,
    load_trajectory,
    require_session_evidence,
)

RESULT_FILE = Path("/app/r4_result.txt")
MOCK_LOG = Path("/logs/agent/e2e_mock_keychain.log")
RESTART_EVIDENCE = Path("/logs/agent/session_e2e_restart_evidence.json")
LOGS_DIR = Path("/logs/agent")


def test_secret_server_stored_masked_and_read_back_after_restart():
    require_session_evidence()
    evidence = load_evidence()
    secrets = injected_values(evidence)
    assert secrets, f"expected delivered secret prompt, got {evidence!r}"

    assert MOCK_LOG.is_file(), (
        "e2e_mock_keychain.log missing — trial did not run with LIBRAGENT_E2E_MOCK_KEYCHAIN=succeed"
    )
    entries = [
        json.loads(line)
        for line in MOCK_LOG.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert any(e.get("op") == "set" and e.get("outcome") == "ok" for e in entries), (
        f"expected successful mock keychain set, got {entries!r}"
    )

    assert RESTART_EVIDENCE.is_file(), "session_e2e_restart_evidence.json missing"
    restart = json.loads(RESTART_EVIDENCE.read_text(encoding="utf-8"))
    assert restart.get("restartExecuted") is True, f"instance restart failed: {restart!r}"

    http_by_name = {p["name"]: p for p in restart.get("http") or [] if isinstance(p, dict)}
    assert http_by_name.get("health", {}).get("status") == 200, (
        f"post-restart /health failed: {http_by_name!r}"
    )

    sql_by_name = {p["name"]: p for p in restart.get("sql") or [] if isinstance(p, dict)}
    rows = (sql_by_name.get("mcp_server") or {}).get("rows") or []
    assert len(rows) == 1, f"expected e2e-r4-readback row after restart, got {rows!r}"
    name, config, secret_names = rows[0]
    assert name == "e2e-r4-readback"
    assert "{{secret:E2E_R4_KEY}}" in str(config), (
        f"expected template placeholder in config, got {config!r}"
    )
    assert "E2E_R4_KEY" in str(secret_names or ""), (
        f"expected E2E_R4_KEY in secret_names, got {secret_names!r}"
    )

    assert RESULT_FILE.is_file(), "r4_result.txt missing"
    assert RESULT_FILE.read_text(encoding="utf-8").strip() == "R4_READBACK_DONE"


def test_secret_literal_never_leaks():
    secrets = injected_values(load_evidence())
    if not secrets:
        return
    secret = secrets[0]
    assert secret not in json.dumps(load_trajectory()), "secret leaked into trajectory"
    if RESTART_EVIDENCE.is_file():
        assert secret not in RESTART_EVIDENCE.read_text(encoding="utf-8"), (
            "secret literal leaked into post-restart evidence"
        )
    if LOGS_DIR.is_dir():
        for artifact in LOGS_DIR.rglob("*"):
            if not artifact.is_file() or artifact.resolve() == EVIDENCE.resolve():
                continue
            assert secret.encode("utf-8") not in artifact.read_bytes(), (
                f"secret literal leaked into {artifact}"
            )
