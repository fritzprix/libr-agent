# LibrAgent HTTP API Documentation

LibrAgent provides a robust HTTP API for remote management of AI agents, sessions, and multi-tool orchestration. This guide is intended for client developers building third-party integrations or headless automation.

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

| Code  | Description                                                                                           |
| :---- | :---------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| `200  | 201`                                                                                                  | **Success**: The request was handled successfully. |
| `400` | **Bad Request**: The request body is malformed or contains invalid values (e.g., non-absolute paths). |
| `404` | **Not Found**: The requested resource (Assistant, Session) does not exist.                            |
| `500` | **Internal Error**: An unexpected server-side error occurred (e.g., DB failure, config corruption).   |

---

## 🚀 Session Management

### Create Session

Creates a new isolated agent session and starts an initial workflow.

- **Method**: `POST`
- **Path**: `/api/sessions`
- **Success Code**: `201 Created`

#### Request Body

```json
{
  "assistantId": "string",
  "name": "string (optional)",
  "workspacePath": "absolute path (optional)",
  "request": "initial user prompt (required)"
}
```

> [!NOTE]
> The API automatically resolves the `model` and `provider` from the user's **Preferred Model** in global settings.

#### Response Body

```json
{
  "id": "session-uuid",
  "name": "Session Name",
  "status": "Busy"
}
```

---

### Get Session Status

Retrieves current session metadata and execution state.

- **Method**: `GET`
- **Path**: `/api/sessions/:id`

#### Response Body

```json
{
  "id": "string",
  "name": "string",
  "status": "Idle | Busy | Paused | Error",
  "model": "string",
  "provider": "string",
  "createdAt": 1739000000000
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

### Get Child Sessions

Retrieves a list of child sessions spawned by a parent session.

- **Method**: `GET`
- **Path**: `/api/sessions/:id/children`

#### Response Body

```json
{
  "childSessions": [
    {
      "id": "string",
      "name": "string",
      "status": "Idle | Busy | Paused | Error",
      "model": "string",
      "provider": "string",
      "createdAt": 1739000000000
    }
  ]
}
```

---

### Resume Session

Resumes a paused session.

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
  "content": "New message text"
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
  "description": "Software development specialist",
  "config": "JSON String"
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
  "version": "0.1.0"
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
