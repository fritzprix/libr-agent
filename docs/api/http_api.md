# LibrAgent HTTP API Documentation

LibrAgent provides a robust HTTP API for remote management of AI agents, sessions, and multi-tool orchestration. This guide is intended for client developers building third-party integrations, automated test runners, or headless benchmark environments (e.g., `tau-bench`, `agent-bench`).

## Server Configuration

- **Default Base URL**: `http://localhost:3030` (configurable in app settings)
- **Content-Type**: `application/json`
- **Access control**:
  - Requests with a non-empty `Origin` header are rejected (`403`). CLI clients (Harbor, libr-delegate, bench scripts) omit `Origin` and are unaffected.
  - When the server is bound to loopback only (default), `Host` must be `localhost`, `127.0.0.1`, or `[::1]` (optional port). This mitigates DNS rebinding.
  - When **Expose to network** is enabled, the Host loopback check is skipped so LAN clients can use the machine IP; Origin rejection still applies.
- **Authentication**:
  - Bearer token authentication supported via `Authorization: Bearer <token>` (`libr_...`).
  - Keys can be injected via `LIBRAGENT_SESSION_API_KEY` or `LIBRAGENT_SESSION_API_KEY_FILE`, or issued dynamically via the interactive agent pairing flow (`POST /api/pairings`).
  - Authentication is required only when an API key is set.
- **CORS**: Cross-origin browser access is not enabled (no `Access-Control-Allow-Origin`).
- **Finding the running backend**: after it binds, the process that hosts this API writes its own record `<database file>.backend.<pid>.json` next to its SQLite database, for example `libragent_v2.db.backend.12345.json`:

  ```json
  { "pid": 12345, "port": 3030, "startedAtMs": 1739000000000 }
  ```

  Every running backend publishes a record of its own, so a second runtime never hides the first. Another process on the same profile scans these records and picks the newest one that is live: `pid` is a running process and the port accepts a connection. Records whose pid is definitively dead are deleted on scan. The records are not a lock: a second app is never refused because of them. A process that finds a live backend does not run startup session recovery, so it never resets that backend's busy or queued sessions.

---

## ⚠️ Error Handling

LibrAgent uses standard HTTP status codes to indicate the success or failure of an API request. All error responses follow a unified JSON schema.

### Error Response Schema

```json
{
  "error": "Detailed explanation of which operation failed and why."
}
```

### Common Status Codes

| Code  | Description                                                                                           |
| :---- | :---------------------------------------------------------------------------------------------------- |
| `200` | **Success**: The request was handled successfully.                                                    |
| `201` | **Created**: The resource (e.g., Session) was successfully created.                                   |
| `400` | **Bad Request**: The request body is malformed or contains invalid values (e.g., non-absolute paths). |
| `404` | **Not Found**: The requested resource (Assistant, Session) does not exist.                            |
| `500` | **Internal Error**: An unexpected server-side error occurred (e.g., DB failure, config corruption).   |

---

## 🚀 Session Management

### Session ID forms (external)

HTTP session APIs use one session id string for responses. Requests accept the
exact storage id, and as a **read-only** legacy fallback also accept a bare
last-10 suffix or optional `session-{suffix}` when it uniquely matches.

| Direction               | Form                                                                 |
| ----------------------- | -------------------------------------------------------------------- |
| **Responses**           | Exact storage id (new sessions: 10 lowercase hex, e.g. `a1b2c3d4e5`) |
| **Requests** (`:id`, …) | Exact id first; unique legacy short / `session-{…}` fallback         |

---

### Create Session

Creates a new isolated agent session and starts an initial workflow.

- **Method**: `POST`
- **Path**: `/api/sessions`
- **Success Code**: `201 Created`

#### Request Body

```json
{
  "assistantId": "string (required)",
  "name": "string (optional)",
  "model": "string (optional, e.g., 'gpt-4o' or 'claude-3-5-sonnet')",
  "provider": "string (optional, e.g., 'openai' or 'anthropic')",
  "workspacePath": "absolute path (optional)",
  "workspaceIsolation": "host | docker (optional)",
  "dockerConfig": {
    "image": "string (optional; default: ghcr.io/fritzprix/libragent-session:<app version>)",
    "env": { "KEY": "VALUE" }
  },
  "executionMode": "normal | yolo | unsafe (optional)",
  "request": "initial user prompt (optional; omit or blank to create an idle session without starting a workflow)",
  "parentSessionId": "string (optional; exact id or unique legacy short ref)",
  "maxDepth": 5,
  "maxFanout": 3,
  "orgId": "string (optional)",
  "orgName": "string (optional)",
  "orgRootSessionId": "string (optional; exact id or unique legacy short ref)"
}
```

> [!NOTE]
> If `model` and `provider` are omitted, the API automatically falls back to the user's **Preferred Model** in global settings.

> [!NOTE]
> `executionMode` is applied **before** the initial workflow starts (or immediately when creating an idle session with no `request`). Use `yolo` for unattended benchmark/automation runs that need standard tools auto-approved, or `unsafe` when hard-approval tools must also run without a human. When omitted, child sessions inherit a non-`normal` parent mode; otherwise the session defaults to `normal`.

#### Execution Mode & Tool Approval Matrix

| Execution Mode | Standard Approval Tools | Hard Approval Tools |
| :--- | :--- | :--- |
| `normal` (default) | **Requires approval** (`workspace__writeFile`, `workspace__strReplace`, `workspace__editFile`, `workspace__editObject`, `workspace__runShell`, `workspace__runInPersistentShell`, `workspace__spawnProcess`, `workspace__runPowerShell`, `workspace__runInPersistentPowerShell`, `workspace__runCmd`, `workspace__runInPersistentCmd`, `workspace__stopProcess`, `scheduled_task__*` write tools, `media__captureScreen`, `desktop__computerControl`) | **Requires hard approval** (`tool__registerServer` except unauthenticated curated catalog presets, `tool__updateServer`, `media__deployAssistPlugin`, and dynamic shell evaluation: `$(...)`, backticks, `<(...)`, `>(...)`, `eval`, and `source`) |
| `yolo` | **Auto-approved** | **Requires hard approval** (waits for explicit approval or a raise to `unsafe`) |
| `unsafe` | **Auto-approved** | **Auto-approved** |

> [!NOTE]
> Destructive commands and sensitive or protected paths (such as `~/.ssh`, `~/.aws`, application database, and restricted system directories) are **blocked** in every execution mode, not routed to hard approval.

> [!NOTE]
> When `request` is omitted or blank, the session is created in `idle` status and no workflow is started. This is useful for API smoke checks that only need to verify configuration (for example `executionMode`) without burning an LLM turn.

#### Response Body

```json
{
  "id": "a1b2c3d4e5",
  "name": "Session Name",
  "status": "Idle | Provisioning",
  "parentSessionId": null,
  "lineageId": "a1b2c3d4e5",
  "depth": 0,
  "maxDepth": 5,
  "maxFanout": 3,
  "orgId": null,
  "orgName": null,
  "orgRootSessionId": null
}
```

`id`, `parentSessionId`, `lineageId`, and `orgRootSessionId` are exact session ids (new sessions: 10 hex).

---

### Get Session Status

Retrieves current session metadata and execution state.

- **Method**: `GET`
- **Path**: `/api/sessions/:id`
- **`:id`**: exact session id, or unique legacy short / `session-{…}` ref

#### Response Body

```json
{
  "id": "a1b2c3d4e5",
  "name": "string",
  "status": "Idle | Busy | Paused | Error | Provisioning",
  "model": "string",
  "provider": "string",
  "assistantId": "string",
  "parentSessionId": "string (nullable)",
  "lineageId": "string",
  "depth": 0,
  "maxDepth": 5,
  "maxFanout": 3,
  "orgId": "string (nullable)",
  "orgName": "string (nullable)",
  "orgRootSessionId": "string (nullable)",
  "isBookmarked": false,
  "createdAt": 1739000000000,
  "updatedAt": 1739000000000,
  "lastViewedAt": 1739000000000,
  "lastMessageAt": 1739000000000,
  "lastAttentionAt": null,
  "lastAttentionReason": "recurringStop | pendingApproval | null",
  "executionMode": "normal | yolo | unsafe",
  "workspaceOverride": "string (nullable)",
  "workspaceIsolation": "host | docker",
  "dockerConfig": null,
  "dockerContainerName": null,
  "dockerHostWorkspacePath": null,
  "statusReason": "string (nullable)",
  "pendingApprovals": [
    {
      "toolCallId": "call-uuid",
      "toolName": "workspace__runShell",
      "reason": "Command substitution requires approval",
      "commandPreview": "echo $(whoami)",
      "approvalKind": "standard | hard",
      "requestId": "approval-request-uuid (optional)"
    }
  ]
}
```

- `lastAttentionReason`: `null`, `"recurringStop"` (a recurring scheduled task stopped or completed), or `"pendingApproval"` (the session is `busy` and waiting on one or more tool approvals listed in `pendingApprovals`).
- `pendingApprovals`: Active tool calls currently awaiting approval while `status` remains `busy`. Each entry includes `toolCallId`, `toolName`, `reason`, `commandPreview`, `approvalKind` (`"standard"` or `"hard"`), and optional channel relay `requestId`.
- `statusReason`: Tells why the session has its current status. When a Docker session cannot prepare its container (for example, a failed image pull or a missing attach container), `status` is `Error` and `statusReason` contains the failure. Session env values, wake tokens, and session API keys are replaced with `[redacted]`. The value stays after an app restart. The next status change clears it.

---

### Update Session Execution Mode

Updates the session's `executionMode` (`normal`, `yolo`, or `unsafe`) using the same authentication guard as session creation. Raising the mode automatically approves and releases any pending approvals covered by the new mode (`yolo` releases `standard` approvals; `unsafe` releases both `standard` and `hard` approvals). Lowering the mode is always allowed.

- **Method**: `PATCH`
- **Path**: `/api/sessions/:id`

#### Request Body

```json
{
  "executionMode": "normal | yolo | unsafe"
}
```

#### Response Body

```json
{
  "id": "a1b2c3d4e5",
  "executionMode": "unsafe",
  "resolvedApprovalIds": ["call-uuid"]
}
```

---

### Terminate Session

Immediately stops any running workflows and cleans up session resources.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/terminate`

#### Response Body

```json
{
  "success": true
}
```

---

### Delete Session

Deletes a session and cascaded descendants from the database and in-memory state.

- **Method**: `DELETE`
- **Path**: `/api/sessions/:id`

#### Response Body

```json
{
  "success": true,
  "deletedIds": ["a1b2c3d4e5", "f6e5d4c3b2"]
}
```

`deletedIds` are exact session ids.

---

### Get Child Sessions

Retrieves a list of child sessions spawned by a parent session.

- **Method**: `GET`
- **Path**: `/api/sessions/:id/children`

#### Response Body

```json
{
  "parentSessionId": "a1b2c3d4e5",
  "count": 2,
  "children": ["1111111111", "2222222222"]
}
```

---

### List Sessions

Lists sessions, most recent activity first.

- **Method**: `GET`
- **Path**: `/api/sessions`
- **Query Parameters** (all optional):
  - `limit`: `number` (default: 50, max: 200)
  - `cursorUpdatedAt`, `cursorId`: the `nextCursor` of the previous page; give both or neither
  - `search`: case-insensitive match on session name/id or assistant name
  - `status`: `idle | busy | paused | error | queued | provisioning`
  - `bookmarked`: `true` for bookmarked sessions only

#### Response Body

```json
{
  "sessions": [
    {
      "id": "a1b2c3d4e5",
      "name": "string",
      "status": "idle",
      "workspaceOverride": "string (nullable)",
      "updatedAt": 1739000000000,
      "lastMessageAt": 1739000000000
    }
  ],
  "nextCursor": { "updatedAt": 1739000000000, "id": "a1b2c3d4e5" }
}
```

Each entry has the session fields of [Get Session Status](#get-session-status), without `pendingApprovals`. `nextCursor` is `null` on the last page.

---

### Session Events (stream)

Follows one session as [server-sent events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events), so a frontend does not have to poll.

- **Method**: `GET`
- **Path**: `/api/sessions/:id/events`
- **Response**: `text/event-stream`

| Event     | Data                                                                                                   | Sent when                                                                                      |
| :-------- | :----------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------- |
| `status`  | The [Get Session Status](#get-session-status) body, including `pendingApprovals` and `pendingUserPrompt` | First, then whenever any field changes                                                         |
| `message` | One [Message Object](#message-object)                                                                  | A message is stored, or a stored message changes. Replace any message you hold with the same `id` |
| `end`     | `{}`                                                                                                   | The session no longer exists. The stream then closes                                           |

Messages that existed when the stream opened are not replayed; read them with [Get Messages](#get-messages). The stream carries what the backend stores. Token-by-token model output is produced by the frontend that runs the LLM call and is not part of this stream.

---

### Resume Session

Loads a paused/crashed session into active memory and resumes the workflow.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/resume`

#### Response Body

```json
{
  "status": "resumed"
}
```

---

## 💬 Messaging

### Send Message

Sends a new user message to an existing session.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/messages`
- **Behavior**:
  - If session is **Idle**: Starts a new workflow immediately.
  - If session is **Busy**: Queues the message for sequential processing.

#### Request Body

```json
{
  "content": "New message text",
  "source": "api" // Optional message source (defaults to "api")
}
```

#### Response Body

```json
{
  "id": "message-uuid",
  "status": "processed | queued"
}
```

---

### Get Messages

Retrieves the message history for a session.

- **Method**: `GET`
- **Path**: `/api/sessions/:id/messages`
- **Query Parameters**:
  - `limit`: `number` (default: 50)

#### Response Body

```json
{
  "messages": [
    {
      "id": "string",
      "role": "user | assistant | tool",
      "content": [
        {
          "type": "text",
          "text": "Hello world"
        }
      ],
      "createdAt": 1739000000000
    }
  ]
}
```

---

## 🔌 Channels & Tool Approvals

These endpoints are crucial for **headless automated testing**. They allow external scripts to simulate remote notifications and programmatic tool permissions.

### Inject Channel Message (Scoped)

Injects a channel-originated system notification into a specific session.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/channel`

#### Request Body

```json
{
  "serverName": "github-webhook",
  "content": "A new PR was opened by user-123",
  "meta": {
    "action": "opened",
    "prNumber": "42"
  }
}
```

#### Response Body

```json
{
  "messageId": "msg-uuid",
  "status": "processed | queued"
}
```

---

### Inject Channel Message (Auto-Route)

Automatically routes a channel message to the uniquely active session listening on that channel.

- **Method**: `POST`
- **Path**: `/api/channel`

#### Request Body

```json
{
  "serverName": "github-webhook",
  "content": "PR #42 updated"
}
```

#### Response Body

```json
{
  "messageId": "msg-uuid",
  "sessionId": "active-session-uuid",
  "sessionName": "Benchmark Task 1",
  "status": "processed"
}
```

---

### Programmatic Tool Approval

Responds to a pending tool execution approval (e.g., executing shell commands in a benchmark) from an external script.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/channel/permission`

#### Request Body

Provide either `toolCallId` (from `GET /api/sessions/:id` `pendingApprovals`) or `requestId` (from a channel permission relay event):

```json
{
  "toolCallId": "call-uuid",
  "requestId": "approval-request-uuid (optional if toolCallId is provided)",
  "behavior": "allow | deny"
}
```

#### Response Body

```json
{
  "requestId": "approval-request-uuid (nullable)",
  "toolCallId": "call-uuid",
  "approved": true
}
```

---

### Answer a User Prompt

Answers or cancels the question a session is waiting on (`ui__askUser`, or a secret request). The waiting prompt is `pendingUserPrompt` in [Get Session Status](#get-session-status); it carries the `requestId` to send back and never carries a secret value.

- **Method**: `POST`
- **Path**: `/api/sessions/:id/channel/prompt`

#### Request Body

```json
{
  "requestId": "prompt-request-uuid",
  "action": "answer | cancel",
  "value": "the answer (required when action is answer)"
}
```

A `requestId` that does not match the waiting prompt returns `404`.

---

## 🤖 Assistants

### List Assistants

Lists all configured assistant roles available on the platform.

- **Method**: `GET`
- **Path**: `/api/assistants`

#### Response Body

```json
{
  "assistants": [
    {
      "id": "string",
      "name": "Coder",
      "description": "Software development specialist",
      "config": "JSON String"
    }
  ]
}
```

---

### Get Assistant

Retrieves details for a specific assistant.

- **Method**: `GET`
- **Path**: `/api/assistants/:id`

#### Response Body

```json
{
  "id": "string",
  "name": "Coder",
  "config": "{...}",
  "created_at": 1739000000000,
  "updated_at": 1739000000000
}
```

---

## 🏥 System

### Health Check

Checks if the server is running, and whether it is ready to run a session.

- **Method**: `GET`
- **Path**: `/api/health`

#### Response Body

```json
{
  "status": "ok",
  "service": "libr-agent-session-api",
  "version": "0.9.31",
  "ready": true,
  "recovery": "pending | ok | failed | skipped"
}
```

- `status: "ok"` only means the HTTP server answered. Wait for `ready: true` before sending work.
- `ready` becomes `true` when startup session recovery has ended.
- `recovery` says how it ended: `ok`; `failed` (sessions from a previous run may still be marked busy); or `skipped` (another live backend owns the sessions, so this process left them alone). `pending` goes with `ready: false`.

---

### Preferred Model

Returns the global preferred model used when session create omits `model`/`provider`.

- **Method**: `GET`
- **Path**: `/api/settings/preferredModel`

#### Response Body

```json
{
  "model": "gpt-4o",
  "provider": "openai",
  "harborModel": "openai/gpt-4o"
}
```

---

### Complexity Model Mapping

Per-level model overrides for sub-agent tools (`agent__spawnSession` / `agent__messageToSession` `complexity` parameter). A `null` level inherits the normal parent → preferred-model chain.

- **Method**: `GET` | `PUT`
- **Path**: `/api/settings/complexityModelMapping`

#### GET Response / PUT Body

```json
{
  "low": { "model": "gpt-4o-mini", "provider": "openai" },
  "normal": null,
  "high": { "model": "claude-opus", "provider": "anthropic" }
}
```

`PUT` validates each level as `null` or `{ model, provider }` (both non-empty strings) and returns the normalized object.

---

## 📦 Data Structures

### Message Object

| Field       | Type                | Description                     |
| :---------- | :------------------ | :------------------------------ |
| `id`        | `string`            | Unique identifier.              |
| `role`      | `string`            | `user`, `assistant`, or `tool`. |
| `content`   | `Array<MCPContent>` | Array of content items.         |
| `createdAt` | `number`            | Unix timestamp (ms).            |

### MCPContent Object

Used inside the message `content` array to support rich output.

```json
{
  "type": "text",
  "text": "The content string",
  "isError": false
}
// OR
{
  "type": "thinking",
  "thinking": "The internal chain of thought",
  "thinkingTime": 1.2
}
```
