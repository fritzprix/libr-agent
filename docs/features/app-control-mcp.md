# App Control MCP (`POST /mcp/control`)

Sessionless **UI chrome** control for LibrAgent desktop. Generic primitives only — compose demos, CI, and Cursor automation in the client.

Not registered on per-session agent tool lists. Does not replace `POST /mcp` / `POST /mcp/{session_id}` (agent builtins).

## Enable

Both flags required:

```bash
# env
export LIBRAGENT_MCP_ENABLE=1
export LIBRAGENT_APP_CONTROL=1

# or CLI
libragent --mcp --app-control
```

For demo filming, prefer **demo profile** (clean DB under
`…/com.fritzprix.libragent-demo`, LLM seeded from `.env.demo`):

```bash
./scripts/run-demo.sh
# or
libragent --demo --mcp --app-control
```

Desktop UI must be open (AppHandle emits `libragent:app-control` to the React bridge).

Endpoint: `http://127.0.0.1:<http_port>/mcp/control`  
Port: Settings HTTP port, or `~/.libragent/http_port`.

## Tools (v1)

| Tool | Args | Effect |
| ---- | ---- | ------ |
| `app__navigate` | `path` (e.g. `/mcp-servers`) | React-router navigate |
| `app__highlight` | `target: "preset"`, `name`, optional `ms` | Open Extensions + highlight recommended preset card |
| `app__install_preset` | `name` (e.g. `ddg-search`) | One-click install zero-config preset (fails if keys/OAuth required) |
| `app__focus_session` | `sessionId` | Navigate to `/agent/{sessionId}` |
| `app__wait_ui` | optional `ms` (default 500), optional `ready.path` hint | Sleep for pacing between steps |

There is **no** composite scenario tool. Demo shot lists live in the bundled [demo-play shot list](../../src-tauri/bundled_skills/demo-play/references/hero-shot-list.md) and operator scripts that call these primitives.

## Cursor MCP config example

```json
{
  "mcpServers": {
    "libragent-app-control": {
      "url": "http://127.0.0.1:3030/mcp/control"
    }
  }
}
```

(Adjust port. Prefer Streamable HTTP / URL transport if your Cursor build supports it; otherwise use a thin stdio bridge that POSTs JSON-RPC.)

## Example JSON-RPC

```bash
PORT=$(cat ~/.libragent/http_port 2>/dev/null || echo 3030)
curl -sS -X POST "http://127.0.0.1:${PORT}/mcp/control" \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"app__highlight","arguments":{"target":"preset","name":"hn","ms":2500}}}'
```

Quiet zero-config presets for one-click install: `hn`, `arxiv`, `ddg-search`, `docx`, …  
Presets with keys/OAuth (e.g. `github`) are rejected by the UI bridge.  
**Demo filming:** do not use `serena` — it spawns a browser dashboard and steals focus.

## Demo composition (script, not a tool)

Use primitives in order — see [demo-play shot list](../../src-tauri/bundled_skills/demo-play/references/hero-shot-list.md):

1. `app__navigate` → `/mcp-servers` (or `app__highlight` a zero-config preset)
2. `app__wait_ui` `{ "ms": 800 }`
3. `app__install_preset` `{ "name": "hn" }`
4. `app__wait_ui` `{ "ms": 1500 }`
5. Create session via `POST /api/sessions` + `app__focus_session`
6. Send the deliverable prompt via session message API / UI

Operator skill: bundled `@skill:demo-play` (`src-tauri/bundled_skills/demo-play/`).

Security: opt-in only; localhost by default unless HTTP expose is enabled. Do not enable app-control on exposed hosts without a network policy.
