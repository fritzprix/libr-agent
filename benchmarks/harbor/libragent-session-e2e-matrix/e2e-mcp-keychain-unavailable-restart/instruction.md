This task verifies the encrypted-fallback MCP secret store across an instance restart when the OS keychain is unavailable (2672 M-5).
The automated harness approves tool calls, answers the secret prompt with a synthetic value, and restarts the isolated instance in place after you finish. Do not connect to the HTTP endpoint; it is intentionally unreachable.

Step 1 — Call `tool__registerServer` with:
- `name`: `"e2e-m5-fallback"`
- `transport`: `{"type": "http", "url": "http://127.0.0.1:9999/mcp?token={{secret:E2E_M5_KEY}}"}`

Step 2 — Write `M5_FALLBACK_DONE` to `/app/m5_result.txt`, then stop.
