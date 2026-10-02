# Session API (curl)

Load when making HTTP calls. Base URL: see SKILL.md (`~/.libragent/http_port` or `3030`).

```bash
PORT=$(cat ~/.libragent/http_port 2>/dev/null || echo 3030)
BASE="http://127.0.0.1:${PORT}"
```

## Health / assistants

```bash
curl -sS "$BASE/api/health"
curl -sS "$BASE/api/assistants"
```

Pick an `assistantId` from the assistants list for create.

## Create session

### Preferred: write JSON to a file, then curl

Long `request` bodies and shell escaping break easily with inline `-d`. Prefer
Python (or similar) writing a file, then validate before POST.

```bash
ASSISTANT_ID="…"          # from GET /api/assistants
WORKSPACE="/absolute/path" # optional
PAYLOAD=/tmp/libr-delegate-create.json

# IMPORTANT: <<'PY' does NOT expand shell vars. Pass paths as Python string
# literals (or use unquoted <<PY carefully). Never reference WORKSPACE as a
# bare Python name inside <<'PY' — that yields NameError → empty file → 400.
python3 - <<PY
import json
from pathlib import Path
Path("$PAYLOAD").write_text(json.dumps({
    "assistantId": "$ASSISTANT_ID",
    "name": "cursor-delegate",
    "workspacePath": "$WORKSPACE",
    "executionMode": "yolo",
    "request": """self-contained task here""",
}, ensure_ascii=False))
PY

# Guard: refuse empty / invalid JSON (empty body → HTTP 400
# "Request body deserialize error: EOF while parsing a value")
test -s "$PAYLOAD" || { echo "empty payload: $PAYLOAD"; exit 1; }
python3 -m json.tool "$PAYLOAD" >/dev/null || { echo "invalid JSON: $PAYLOAD"; exit 1; }

curl -sS -X POST "$BASE/api/sessions" \
  -H 'content-type: application/json' \
  -d @"$PAYLOAD"
```

### Inline JSON (short payloads only)

```bash
curl -sS -X POST "$BASE/api/sessions" \
  -H 'content-type: application/json' \
  -d '{
    "assistantId": "'"$ASSISTANT_ID"'",
    "name": "cursor-delegate",
    "request": "'"$TASK"'",
    "workspacePath": "'"$WORKSPACE"'",
    "executionMode": "yolo"
  }'
```

Optional lineage:

```json
"parentSessionId": "<exact parent id>"
```

Optional caps: `maxDepth`, `maxFanout`.

Response includes exact `id` (new sessions: 10 hex). Use that id for all later calls.

Idle create (no LLM until message):

```bash
curl -sS -X POST "$BASE/api/sessions" \
  -H 'content-type: application/json' \
  -d "{\"assistantId\":\"$ASSISTANT_ID\",\"executionMode\":\"yolo\"}"
```

## Send / follow-up message

```bash
curl -sS -X POST "$BASE/api/sessions/$SID/messages" \
  -H 'content-type: application/json' \
  -d '{"content":"…","source":"api"}'
```

Idle → starts workflow. Busy → queues.

## Status / messages / children

```bash
curl -sS "$BASE/api/sessions/$SID"
curl -sS "$BASE/api/sessions/$SID/messages?limit=50"
curl -sS "$BASE/api/sessions/$SID/children"
```

Statuses: `Idle` | `Busy` | `Paused` | `Error` | `Provisioning`.

## Extract transcript (before delete)

Session API has no dedicated trace/export route. Dump message history while
the session still exists (default `limit` is 50):

```bash
OUT=".libragent/work/libr-delegate-${SID}-messages.json"
mkdir -p "$(dirname "$OUT")"
curl -sS "$BASE/api/sessions/$SID/messages?limit=500" > "$OUT"
```

Use the latest assistant message as the handoff result; keep `$OUT` when you
need tool/role history after cleanup.

## Resume / terminate / delete

For one-shot delegates, prefer this order: **extract → terminate → delete**.

```bash
curl -sS -X POST "$BASE/api/sessions/$SID/resume"
curl -sS -X POST "$BASE/api/sessions/$SID/terminate"
curl -sS -X DELETE "$BASE/api/sessions/$SID"
```

Delete cascades descendants; response includes `deletedIds`. After DELETE,
`GET …/messages` is gone — do not delete first if you still need the dump.

## Channel inject (optional)

Scoped wake alternative to messages:

```bash
curl -sS -X POST "$BASE/api/sessions/$SID/channel" \
  -H 'content-type: application/json' \
  -d '{"serverName":"cursor-delegate","content":"event summary","meta":{}}'
```

Prefer `/messages` for normal task handoffs.

## Polling hint

Sleep with increasing backoff while `status` is `Busy` or `Provisioning`.
Respect any rate-limit / backoff headers if present. Full schema:
`docs/api/http_api.md`.
