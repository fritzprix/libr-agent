#!/usr/bin/env bash
# Wipe assistant-scoped Knowledge for the knowledge-bridge Harbor suite.
# Safe to re-run. Does not delete sessions or assistants.
set -euo pipefail

API_URL="${LIBRAGENT_API_URL:-http://localhost:${LIBRAGENT_HTTP_PORT:-$(cat "$HOME/.libragent/http_port" 2>/dev/null || echo 3030)}/api}"
API_URL="${API_URL%/}"
export API_URL
export ASSISTANT_ID="${LIBRAGENT_ASSISTANT_ID:-}"
export ASSISTANT_NAME="${LIBRAGENT_KNOWLEDGE_BRIDGE_ASSISTANT_NAME:-Harbor Knowledge Bridge}"
export DB_PATH="${LIBRAGENT_DB_PATH:-}"

python3 <<'PY'
from __future__ import annotations

import json
import os
import sqlite3
import sys
import urllib.request
from pathlib import Path

api = os.environ["API_URL"].rstrip("/")
assistant_id = os.environ.get("ASSISTANT_ID", "").strip()
assistant_name = os.environ.get("ASSISTANT_NAME", "Harbor Knowledge Bridge")
db_override = os.environ.get("DB_PATH", "").strip()


def http_get(url: str):
    with urllib.request.urlopen(url, timeout=15) as resp:
        return json.loads(resp.read().decode())


if not assistant_id:
    try:
        payload = http_get(f"{api}/assistants")
    except Exception as exc:
        print(
            f"ERROR: cannot list assistants at {api}/assistants: {exc}",
            file=sys.stderr,
        )
        sys.exit(1)
    if isinstance(payload, dict) and isinstance(payload.get("assistants"), list):
        assistants = payload["assistants"]
    elif isinstance(payload, list):
        assistants = payload
    else:
        print("ERROR: unexpected /assistants payload", file=sys.stderr)
        sys.exit(1)
    match = next((a for a in assistants if a.get("name") == assistant_name), None)
    if match is None:
        match = next((a for a in assistants if a.get("name") == "Coding Expert"), None)
    if match is None and assistants:
        match = assistants[0]
    if match is None or not match.get("id"):
        print("ERROR: no assistant available to wipe", file=sys.stderr)
        sys.exit(1)
    assistant_id = match["id"]
    print(f"Resolved assistant_id={assistant_id} name={match.get('name')!r}")

candidates: list[Path] = []
if db_override:
    candidates.append(Path(db_override))
else:
    home = Path.home()
    candidates.extend(
        [
            # Prefer prod DB (Harbor desktop usually attaches here).
            home / ".local/share/com.fritzprix.libragent/libragent_v2.db",
            home / ".local/share/com.fritzprix.libragent/libragent_v2.dev.db",
            home
            / "Library/Application Support/com.fritzprix.libragent/libragent_v2.db",
            home
            / "Library/Application Support/com.fritzprix.libragent/libragent_v2.dev.db",
        ]
    )

db_path = next((p for p in candidates if p.is_file() and p.stat().st_size > 0), None)
if db_path is None:
    print("ERROR: SQLite DB not found. Set LIBRAGENT_DB_PATH.", file=sys.stderr)
    for p in candidates:
        print(f"  missing: {p}", file=sys.stderr)
    sys.exit(1)

print(f"Wiping knowledge for assistant_id={assistant_id} in {db_path}")
conn = sqlite3.connect(str(db_path))
try:
    cur = conn.cursor()
    tables = {
        r[0]
        for r in cur.execute(
            "SELECT name FROM sqlite_master WHERE type='table'"
        ).fetchall()
    }

    if "knowledge_chunks_v2" not in tables:
        print("ERROR: knowledge_chunks_v2 table missing", file=sys.stderr)
        sys.exit(1)

    chunk_ids = [
        r[0]
        for r in cur.execute(
            "SELECT id FROM knowledge_chunks_v2 WHERE assistant_id = ?",
            (assistant_id,),
        ).fetchall()
    ]
    print(f"  chunks before: {len(chunk_ids)}")

    if chunk_ids:
        placeholders = ",".join("?" for _ in chunk_ids)
        if "knowledge_chunk_entities" in tables:
            cur.execute(
                f"DELETE FROM knowledge_chunk_entities WHERE chunk_id IN ({placeholders})",
                chunk_ids,
            )
        if "knowledge_vectors" in tables:
            cur.execute(
                f"DELETE FROM knowledge_vectors WHERE rowid IN ({placeholders})",
                chunk_ids,
            )
        cur.execute(
            "DELETE FROM knowledge_chunks_v2 WHERE assistant_id = ?",
            (assistant_id,),
        )

    if "knowledge_relationships" in tables:
        cur.execute(
            "DELETE FROM knowledge_relationships WHERE assistant_id = ?",
            (assistant_id,),
        )
    if "knowledge_entities" in tables:
        cur.execute(
            "DELETE FROM knowledge_entities WHERE assistant_id = ?",
            (assistant_id,),
        )

    if "knowledge" in tables:
        try:
            cur.execute(
                "DELETE FROM knowledge WHERE assistant_id = ?",
                (assistant_id,),
            )
        except sqlite3.Error:
            pass

    conn.commit()
    remaining = cur.execute(
        "SELECT COUNT(*) FROM knowledge_chunks_v2 WHERE assistant_id = ?",
        (assistant_id,),
    ).fetchone()[0]
    print(f"  chunks after: {remaining}")
finally:
    conn.close()

print("OK: knowledge wipe complete")
PY
