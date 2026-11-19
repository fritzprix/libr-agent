This task verifies MCP secret storage under the mock/fallback keychain backend
(2565 M-7 / 2672 M-4 & 2672 M-5 unavailable-keychain half). An automated harness
approves the registration and answers the secret prompt with a synthetic value.
Do not verify or connect to the HTTP endpoint; it is intentionally unreachable.

Step 1 — register an HTTP MCP server with a `{{secret:NAME}}` template:

Call `tool__registerServer` with:
- `name`: `"e2e-keychain-fallback"`
- `transport`: `{"type": "http", "url": "http://127.0.0.1:9999/mcp?key={{secret:E2E_KEYCHAIN_SECRET}}"}`

Step 2 — record completion:

Write `KEYCHAIN_FLOW_DONE` to `keychain_result.txt` with `workspace__writeFile`, then stop.
