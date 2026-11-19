This task verifies the MCP secret template lifecycle (2565 M-2, M-6, M-8, X-6).
The automated harness approves tool calls and answers each secret prompt with a
distinct synthetic value. Do not verify or connect to the HTTP endpoint; it is
intentionally unreachable.

Step 1 — register an HTTP MCP server with a URL secret template:

Call `tool__registerServer` with:
- `name`: `"e2e-secret-lifecycle"`
- `transport`: `{"type": "http", "url": "http://127.0.0.1:9999/mcp?token={{secret:E2E_LIFECYCLE_KEY}}"}`

Step 2 — update the server with `reenterSecrets` so the secret is prompted again:

Call `tool__updateServer` for `"e2e-secret-lifecycle"` with:
- `transport`: `{"type": "http", "url": "http://127.0.0.1:9999/mcp?token={{secret:E2E_LIFECYCLE_KEY}}"}`
- `reenterSecrets`: `["E2E_LIFECYCLE_KEY"]`

Step 3 — confirm the MCP secret store is separate from shell `storeAs` env vars:

Run `workspace__runShell` with command:
`test -z "$E2E_LIFECYCLE_KEY" && echo SEPARATE > store_separate.txt`

Then stop.
