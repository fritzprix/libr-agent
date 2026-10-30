/**
 * Generates an agent-agnostic markdown instruction prompt for pairing an external AI agent
 * with LibrAgent's Session API.
 *
 * @param httpPort Current configured HTTP port for fallback
 */
export function getAgentIntegrationInstruction(
  httpPort: number = 3030,
): string {
  return `# LibrAgent Session API Integration Instructions

You are integrating with LibrAgent's Session API running locally on this machine. Follow these steps autonomously:

1. **Locate HTTP Port**:
   - Check file: \`~/.libragent/http_port\` (or \`%USERPROFILE%/.libragent/http_port\` on Windows).
   - If present, read the port number from that file. Otherwise, default to port \`${httpPort}\`.

2. **Generate a 1-Time Pairing Code**:
   - Generate a 6-character code formatted as \`xxx-yyy\` using alphanumeric characters (e.g. \`a7b-3k9\`).

3. **Initiate Pairing Request**:
   - Send an HTTP POST request:
     - URL: \`http://127.0.0.1:<PORT>/api/pairings\`
     - Method: \`POST\`
     - Headers: \`{"Content-Type": "application/json"}\`
     - Body:
\`\`\`json
{
  "name": "<Your Agent Name>",
  "code": "<GENERATED_CODE>"
}
\`\`\`

4. **Prompt the User**:
   - Inform the user:
     "LibrAgent connection requested. Please enter code [<GENERATED_CODE>] in LibrAgent to approve pairing."

5. **Wait for Approval**:
   - The POST request will block until the user enters the code in LibrAgent (or times out after 120s).
   - When approved, LibrAgent responds with:
\`\`\`json
{
  "apiKey": "libr_...",
  "name": "<Your Agent Name>",
  "id": "..."
}
\`\`\`

6. **Store API Key & Configure Integration**:
   - Save the returned \`apiKey\` to your local workspace or config.
   - Use header \`Authorization: Bearer <apiKey>\` on all subsequent Session API calls (e.g. \`POST /api/sessions\`, \`POST /api/sessions/:id/messages\`).
   - Create or configure integration scripts/skills (e.g. \`libr-delegate\`) to delegate sub-tasks to LibrAgent.
`;
}
