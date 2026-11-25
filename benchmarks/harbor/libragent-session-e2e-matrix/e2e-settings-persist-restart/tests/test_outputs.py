"""2565 O-1 (restart half): a setting written before restart survives in-place restart."""

import json
from pathlib import Path

from verifier_lib import require_session_evidence

MARKER = Path("/app/o1_ready.txt")
RESTART_EVIDENCE = Path("/logs/agent/session_e2e_restart_evidence.json")
SENTINEL_MODEL = "e2e-o1-restart-sentinel"


def test_written_setting_survives_in_place_restart():
    require_session_evidence()
    assert MARKER.is_file(), "o1_ready.txt missing"
    assert MARKER.read_text(encoding="utf-8").strip() == "SETTINGS_RESTART_READY"

    assert RESTART_EVIDENCE.is_file(), "session_e2e_restart_evidence.json missing"
    restart = json.loads(RESTART_EVIDENCE.read_text(encoding="utf-8"))

    pre_by_name = {p["name"]: p for p in restart.get("preRestart") or [] if isinstance(p, dict)}
    pre_put = pre_by_name.get("write_complexity_mapping") or {}
    assert pre_put.get("status") == 200, f"pre-restart settings PUT failed: {pre_put!r}"
    assert SENTINEL_MODEL in json.dumps(pre_put.get("json") or {}), (
        f"pre-restart PUT response missing sentinel: {pre_put!r}"
    )

    assert restart.get("restartExecuted") is True, f"instance restart failed: {restart!r}"

    http_by_name = {p["name"]: p for p in restart.get("http") or [] if isinstance(p, dict)}
    assert http_by_name.get("health", {}).get("status") == 200, (
        f"post-restart /health failed: {http_by_name!r}"
    )
    post_get = http_by_name.get("complexity_mapping") or {}
    assert post_get.get("status") == 200, (
        f"post-restart /settings/complexityModelMapping failed: {post_get!r}"
    )
    post_low = (post_get.get("json") or {}).get("low") or {}
    assert post_low.get("model") == SENTINEL_MODEL and post_low.get("provider") == "openai", (
        f"post-restart complexityModelMapping did not preserve sentinel: {post_get!r}"
    )

    sql_by_name = {p["name"]: p for p in restart.get("sql") or [] if isinstance(p, dict)}
    rows = (sql_by_name.get("complexity_row") or {}).get("rows") or []
    assert len(rows) == 1 and rows[0][0] == "complexityModelMapping", (
        f"expected complexityModelMapping row in settings table after restart, got {rows!r}"
    )
    assert SENTINEL_MODEL in str(rows[0][1]), (
        f"settings SQL row missing sentinel after restart: {rows!r}"
    )
