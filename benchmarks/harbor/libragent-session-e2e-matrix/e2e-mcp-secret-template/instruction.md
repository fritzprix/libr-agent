Register a new MCP server using the mcp-installer flow.

Server details:

- name: `e2e-fake-weather`
- transport: HTTP, URL `http://127.0.0.1:9999/mcp`
- it requires an API key sent as the header `Authorization: Bearer <KEY>`,
  and the key's environment name is `E2E_WEATHER_KEY`

Rules:

- Follow the MCP secret convention: write the secret as a
  `{{secret:NAME}}` template value, never as a literal value.
- Never ask me for the secret value — in chat or with a prompt. If the
  registration flow needs user input, it must only reference the template.
- Do not verify or start the server; the endpoint is unreachable on
  purpose. Registration alone is the deliverable.
- When done, write exactly `REGISTERED` to `mcp_registered.txt`.
