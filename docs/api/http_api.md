# LibrAgent HTTP API Documentation

LibrAgent provides a robust HTTP API for remote management of AI agents, sessions, and multi-tool orchestration. This guide is intended for client developers building third-party integrations, automated test runners, or headless benchmark environments (e.g., `tau-bench`, `agent-bench`).

## Server Configuration

- **Default Base URL**: `http://localhost:3000` (configurable in app settings)
- **Content-Type**: `application/json`
- **CORS**: `Allow-Any-Origin` (Permissive for local development)

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

| Code  | Description                                                                                                                                                                          |
| :---- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `200` | **Success**: The request was handled successfully.                                                                                                                                   |
| `201` | **Created**: The resource (e.g., Session) was successfully created.                                                                                                                  |
| `400` | **Bad Request**: The request body is malformed, contains invalid values (e.g., non-absolute paths), or a session reference is **ambiguous** (short token matches multiple sessions). |
| `404` | **Not Found**: The requested resource (Assistant, Session) does not exist.                                                                                                           |
| `500` | **Internal Error**: An unexpected server-side error occurred (e.g., DB failure, config corruption).                                                                                  |

---

## 🚀 Session Management

### Session ID forms (external)

HTTP session APIs treat session identifiers as **external** references:

| Direction                                                              | Form                                                                                                                                            |
| ---------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Responses**                                                          | Short token only (last 10 characters of the unique part). No `session-` prefix. Example: `a1b2c3d4e5`                                           |
| **Requests** (path `:id`, body `parentSessionId` / `orgRootSessionId`) | Full storage id, bare short token, or optional `session-{short}` — all accepted. Exact storage match wins; ambiguous short tokens return `400`. |

Internal DB / Tauri UI keys are unchanged (legacy `session-<long>` or bare 10-hex spawn ids).

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
    "image": "string (optional)",
    "env": { "KEY": "VALUE" }
  },
  "executionMode": "normal | yolo | unsafe (optional)",
  "request": "initial user prompt (optional; omit or blank to create an idle session without starting a workflow)",
  "parentSessionId": "string (optional; storage id, short token, or session-{short})",
  "maxDepth": 5,
  "maxFanout": 3,
  "orgId": "string (optional)",
  "orgName": "string (optional)",
  "orgRootSessionId": "string (optional; storage id, short token, or session-{short})"
}
```

> [!NOTE]
> If `model` and `provider` are omitted, the API automatically falls back to the user's **Preferred Model** in global settings.

> [!NOTE]
> `executionMode` is applied **before** the initial workflow starts (or immediately when creating an idle session with no `request`). Use `yolo` for unattended benchmark/automation runs that need standard tools auto-approved, or `unsafe` when hard-approval tools must also run without a human. When omitted, child sessions inherit a non-`normal` parent mode; otherwise the session defaults to `normal`.

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

`id`, `parentSessionId`, `lineageId`, and `orgRootSessionId` are always short display tokens (no `session-` prefix).

---

### Get Session Status

Retrieves current session metadata and execution state.

- **Method**: `GET`
- **Path**: `/api/sessions/:id`
- **`:id`**: storage id, short token, or optional `session-{short}`

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
  "lastAttentionReason": null,
  "executionMode": "normal | yolo | unsafe",
  "workspaceOverride": "string (nullable)",
  "workspaceIsolation": "host | docker",
  "dockerConfig": null,
  "dockerContainerName": null,
  "dockerHostWorkspacePath": null
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

`deletedIds` use short display tokens.

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

```json
{
  "requestId": "approval-request-uuid",
  "behavior": "yolo | normal | unsafe | deny"
}
```

#### Response Body

```json
{
  "requestId": "approval-request-uuid",
  "toolCallId": "call-uuid",
  "approved": true
}
```

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

Checks if the server is running and reachable.

- **Method**: `GET`
- **Path**: `/api/health`

#### Response Body

```json
{
  "status": "ok",
  "service": "libr-agent-session-api"
}
```

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
