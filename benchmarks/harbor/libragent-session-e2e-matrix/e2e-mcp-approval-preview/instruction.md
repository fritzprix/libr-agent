This task verifies MCP registration approval preview masking (2672 M-3) and
cancelling the secret prompt without saving a server row (2565 M-4).

Step 1 — attempt to register an HTTP MCP server with both a literal query param,
a `{{secret:NAME}}` query param, and a URL fragment:

Call `tool__registerServer` with:
- `name`: `"e2e-preview-cancel"`
- `transport`: `{"type": "http", "url": "https://127.0.0.1:9999/mcp?literal_key=plainSecret123&api_key={{secret:E2E_PREVIEW_KEY}}#fragSecret456"}`

The automated harness will approve the registration tool call, then CANCEL the
subsequent secret entry prompt for `E2E_PREVIEW_KEY`. Do not retry registration
after cancellation.

Step 2 — verify no server row was saved:

Call `tool__list` with `scope: "external"`. Confirm `"e2e-preview-cancel"` is NOT
in the returned external server list.

Step 3 — record completion:

Write exactly `NOT_REGISTERED` to `mcp_cancel_result.txt` with
`workspace__writeFile`, then stop.
