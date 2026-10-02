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

## Resume / terminate / delete

```bash
curl -sS -X POST "$BASE/api/sessions/$SID/resume"
curl -sS -X POST "$BASE/api/sessions/$SID/terminate"
curl -sS -X DELETE "$BASE/api/sessions/$SID"
```

Delete cascades descendants; response includes `deletedIds`.

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
