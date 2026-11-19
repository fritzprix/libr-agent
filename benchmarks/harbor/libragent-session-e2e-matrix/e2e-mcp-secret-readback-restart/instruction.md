This task verifies that a `{{secret:NAME}}` MCP server registration is stored masked and survives an in-place instance restart (2672 R-4).
The automated harness approves tool calls, answers the secret prompt with a synthetic value, and restarts the isolated instance in place after you finish. Do not connect to the HTTP endpoint; it is intentionally unreachable.

Step 1 — Call `tool__registerServer` with:
- `name`: `"e2e-r4-readback"`
- `transport`: `{"type": "http", "url": "http://127.0.0.1:9999/mcp?api_key={{secret:E2E_R4_KEY}}"}`

Step 2 — Write `R4_READBACK_DONE` to `/app/r4_result.txt`, then stop.
