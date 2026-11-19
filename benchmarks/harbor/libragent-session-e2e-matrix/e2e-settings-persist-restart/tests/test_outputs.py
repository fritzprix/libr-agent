"""2565 O-1 (restart half): settings persist across an in-place isolated restart."""

import json
from pathlib import Path

from verifier_lib import require_session_evidence

MARKER = Path("/app/o1_ready.txt")
RESTART_EVIDENCE = Path("/logs/agent/session_e2e_restart_evidence.json")


def test_settings_survive_in_place_restart():
    require_session_evidence()
    assert MARKER.is_file(), "o1_ready.txt missing"
    assert MARKER.read_text(encoding="utf-8").strip() == "SETTINGS_RESTART_READY"

    assert RESTART_EVIDENCE.is_file(), "session_e2e_restart_evidence.json missing"
    restart = json.loads(RESTART_EVIDENCE.read_text(encoding="utf-8"))
    assert restart.get("restartExecuted") is True, f"instance restart failed: {restart!r}"

    http_by_name = {p["name"]: p for p in restart.get("http") or [] if isinstance(p, dict)}
    assert http_by_name.get("health", {}).get("status") == 200, (
        f"post-restart /health failed: {http_by_name!r}"
    )
    assert http_by_name.get("preferred_model", {}).get("status") == 200, (
        f"post-restart /settings/preferredModel failed: {http_by_name!r}"
    )

    sql_by_name = {p["name"]: p for p in restart.get("sql") or [] if isinstance(p, dict)}
    rows = (sql_by_name.get("settings_count") or {}).get("rows") or []
    assert rows and rows[0][0] >= 1, (
        f"expected seeded settings rows to survive restart, got {rows!r}"
    )
