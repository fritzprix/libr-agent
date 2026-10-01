# App Control reference (demo-play)

## Enable

```bash
libragent --mcp --app-control
# or
export LIBRAGENT_MCP_ENABLE=1
export LIBRAGENT_APP_CONTROL=1
```

Desktop window must be open. Endpoint: `http://127.0.0.1:<port>/mcp/control`  
Port file: `~/.libragent/http_port` (default `3030`).

Probe:

```bash
PORT=$(cat ~/.libragent/http_port 2>/dev/null || echo 3030)
curl -sS -X POST "http://127.0.0.1:${PORT}/mcp/control" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

## Tools

| Tool | Args | Notes |
| ---- | ---- | ----- |
| `app__navigate` | `path` starting with `/`, not `//` | e.g. `/mcp-servers`, `/settings`, `/agent` |
| `app__highlight` | `target:"preset"`, `name`, optional `ms` | Opens Extensions + ring highlight |
| `app__install_preset` | `name` | Zero-config only; FE rejects key/OAuth presets |
| `app__focus_session` | `sessionId` | → `/agent/{sessionId}` |
| `app__wait_ui` | optional `ms` (default 500, max 30000) | Pacing sleep |

Emit is fire-and-forget from Rust; install validation runs in the UI bridge.
Errors for bad presets appear in LibrAgent webview logs, while MCP often returns OK after emit.

## Quiet presets (hero-safe)

Use for one-click Install on film: `hn`, `arxiv`, `ddg-search`, `docx`, `yahoo-finance`.

**Avoid on hero takes**

| Preset | Why |
| ------ | --- |
| `serena` | Starts a local web dashboard (`127.0.0.1:…/dashboard`) and steals OS focus |
| `github`, `exa`, `brave-search`, … | Keys/OAuth — one-click rejected |
| `comfyui` | Heavy / external UI surface |

If Install must be visible and quiet presets are already installed, remove one
from Extensions first — do not pick a noisy “free” preset.

## Session HTTP (after chrome)

- `POST /api/sessions` — body needs `assistantId`; optional `workspacePath`, `request`, `executionMode`
- `POST /api/sessions/:id/messages` — `{ "content": "..." }` when sending after idle create
- Docs: repo `docs/api/http_api.md`

List assistants: `GET /api/assistants`
