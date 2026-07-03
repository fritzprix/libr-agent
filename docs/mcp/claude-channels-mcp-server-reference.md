# LibrAgent Claude Channels — MCP Server Reference Guide

> **Protocol version:** 0.1 (bridge-first, native path in progress)
> **Last updated:** 2026-06-26

## 1. Overview

LibrAgent implements a **Claude Channels**-compatible protocol that lets external MCP servers push real-time notifications into active agent sessions. The protocol has two parts:

| Part                           | Direction              | Method                                   | Status          |
| ------------------------------ | ---------------------- | ---------------------------------------- | --------------- |
| **Bridge (supported)**         | External → LibrAgent   | Tauri command or HTTP API                | ✅ Production   |
| **Native stdio (in progress)** | MCP Server → LibrAgent | `notifications/claude/channel` via stdio | 🚧 Experimental |

This guide covers both paths. For production integration, use the **bridge** path. For direct MCP server integration, the **native** path is available behind the `claude/channel` server capability.

---

## 2. Server Capability Advertisement

MCP servers must advertise channel support during the `initialize` handshake. LibrAgent inspects the `experimental` capabilities map:

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "2024-11-05",
    "capabilities": { ... },
    "clientInfo": { "name": "MyServer", "version": "1.0.0" }
  }
}
```

**Server response** must include:

```json
{
  "jsonrpc": "2.0",
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": { ... },
    "serverInfo": {
      "name": "MyServer",
      "version": "1.0.0",
      "instructions": "Optional instructions shown to the agent."
    },
    "experimental": {
      "claude/channel": {},
      "claude/channel/permission": {}
    }
  }
}
```

| Capability                  | Purpose                                            |
| --------------------------- | -------------------------------------------------- |
| `claude/channel`            | Server can push messages into agent sessions       |
| `claude/channel/permission` | Server can relay tool approval verdicts (optional) |

**LibrAgent behavior:**

- Reads `instructions` from `serverInfo` → injects into the agent system prompt under `## Channels`
- Reads `claude/channel` → marks server as channel-capable
- Reads `claude/channel/permission` → enables outbound permission relay

---

## 3. Outbound Channel Messages (MCP Server → LibrAgent)

MCP servers send channel messages as **JSON-RPC 2.0 notifications** on the server's **stdout** stream. Each message is a single line of JSON followed by a newline.

### 3.1 Message Format

```json
{
  "jsonrpc": "2.0",
  "method": "claude/channel",
  "params": {
    "content": "Hello from the channel!",
    "meta": {
      "chat_id": "12345",
      "sender_name": "Alice",
      "sender_id": "42"
    }
  }
}
```

| Field            | Type   | Required | Description                                                   |
| ---------------- | ------ | -------- | ------------------------------------------------------------- |
| `method`         | string | ✅       | Always `"claude/channel"` or `"notifications/claude/channel"` |
| `params.content` | string | ✅       | The message body (plain text)                                 |
| `params.meta`    | object | ❌       | Key-value metadata. Values are coerced to strings.            |

**Method variants:** Both `claude/channel` and `notifications/claude/channel` are accepted.

### 3.2 Meta Value Coercion

Non-string values in `meta` are automatically converted:

| JSON Type          | Coerced Value                             |
| ------------------ | ----------------------------------------- |
| `string`           | As-is                                     |
| `number`           | `number.toString()` (e.g., `42` → `"42"`) |
| `boolean`          | `"true"` / `"false"`                      |
| `null`             | `""` (empty string)                       |
| `array` / `object` | `JSON.stringify()` result                 |

### 3.3 Transport Details

- **Transport:** stdio (line-delimited JSON)
- **Encoding:** UTF-8
- **Line ending:** `\n` (CR `\r` is stripped)
- **Max line length:** No hard limit on the codec; oversized lines are discarded
- **Message framing:** One JSON object per line

### 3.4 Content Size Limits

| Limit                       | Value                        | Behavior                                                        |
| --------------------------- | ---------------------------- | --------------------------------------------------------------- |
| `MAX_CHANNEL_CONTENT_BYTES` | **8,192 bytes**              | Messages exceeding this are silently dropped with a `warn!` log |
| Channel event buffer        | **1,024 events** per session | New events are dropped when full; metrics logged every 60s      |

**Recommendation:** Keep `content` well under 8 KB. For large payloads, send multiple messages.

---

## 4. Inbound Channel Messages (LibrAgent → Agent)

When LibrAgent receives a channel notification, it formats the content as an **XML block** and injects it as a `role: "user"` message into the agent's conversation.

### 4.1 XML Payload Format

```xml
<channel source="telegram" chat_id="12345" sender_name="Alice">
Hello from the channel!

[/channel_meta]
sender_id=42
[/channel_meta]
</channel>
```

**Structure:**

```xml
<channel {safe_attributes}>
{escaped_content}

{unsafe_meta_as_key=value_pairs}
</channel>
```

### 4.2 Attribute Safety Filtering

LibrAgent applies strict filtering to meta keys before placing them as XML attributes:

**Blocked keys (case-insensitive):**

| Category            | Blocked Values                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `source`            | `source` (always reserved for `server_name`)                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `style`             | `style` (XSS risk)                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| HTML event handlers | `onclick`, `onerror`, `onload`, `onmouseover`, `onfocus`, `onblur`, `onchange`, `onsubmit`, `onreset`, `onselect`, `onkeydown`, `onkeypress`, `onkeyup`, `onabort`, `oncanplay`, `ontoggle`, `onanimationend`, `ontransitionend`, `onpointerdown`, `onpointerup`, `onpaste`, `oncut`, `oncopy`, `ondrag`, `ondrop`, `onscroll`, `onwheel`, `onresize`, `onbeforeunload`, `onhashchange`, `onmessage`, `onoffline`, `ononline`, `onpagehide`, `onpageshow`, `onpopstate`, `onstorage`, `onunload` |

**Attribute naming rules:**

1. Must start with ASCII letter or `_`
2. Rest must be ASCII alphanumeric, `_`, or `-`
3. Empty keys are rejected

**Fallback behavior:** Keys that fail safety checks or naming rules are moved to the `[channel_meta]` body block instead of being emitted as attributes.

### 4.3 XML Escaping

| Character | Attribute context | Text context |
| --------- | ----------------- | ------------ |
| `&`       | `&amp;`           | `&amp;`      |
| `"`       | `&quot;`          | (unchanged)  |
| `<`       | `&lt;`            | `&lt;`       |
| `>`       | `&gt;`            | `&gt;`       |

### 4.4 Agent Message Structure

The XML block becomes a `Message` object with:

```typescript
{
  role: "user",
  content: [{ type: "text", text: "<channel>...</channel>" }],
  source: "channel",
  metadata: {
    channel: {
      serverName: "telegram",
      meta: { chat_id: "12345", ... }
    }
  }
}
```

---

## 5. Permission Relay

When a tool requires user approval, LibrAgent can relay the request to channel-capable servers. This enables bot-mediated approvals (e.g., Telegram bot confirms tool execution).

### 5.1 Permission Request (LibrAgent → MCP Server)

LibrAgent sends a **client notification** to servers advertising `claude/channel/permission`:

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/claude/channel/permission_request",
  "params": {
    "request_id": "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
    "tool_name": "workspace__writeFile",
    "description": "Claude requested approval to run tool workspace__writeFile with the provided arguments",
    "input_preview": "path: /tmp/test.txt\ncontent: hello world ..."
  }
}
```

| Field           | Type   | Description                                                      |
| --------------- | ------ | ---------------------------------------------------------------- |
| `request_id`    | string | UUID v4 (32-char lowercase hex). Unique per approval request.    |
| `tool_name`     | string | The MCP tool name requiring approval                             |
| `description`   | string | Human-readable description of the approval request               |
| `input_preview` | string | First 200 chars of tool arguments (truncated with `…` separator) |

### 5.2 Permission Verdict (MCP Server → LibrAgent)

The MCP server responds with a verdict notification on **stdout**:

```json
{
  "jsonrpc": "2.0",
  "method": "claude/channel/permission",
  "params": {
    "request_id": "a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
    "behavior": "allow"
  }
}
```

**Accepted behaviors:**

| Value     | Effect                      |
| --------- | --------------------------- |
| `"allow"` | Tool execution proceeds     |
| `"deny"`  | Tool execution is cancelled |

**Invalid values** are rejected with an error: `"Invalid channel permission behavior: {value} (expected 'allow' or 'deny')"`

### 5.3 Permission Flow Diagram

```
  Agent Session              LibrAgent Backend              MCP Server
       |                           |                             |
       |  Tool call requires       |                             |
       |  approval                 |                             |
       |-------------------------->|                             |
       |                           |  Emit channelPermissionRequest event
       |                           |  to frontend (UI)
       |                           |                             |
       |                           |  Send permission_request    |
       |                           |  notification               |
       |                           |---------------------------->|
       |                           |                             |
       |                           |  (Server surfaces to user)  |
       |                           |                             |
       |                           |  < Verdict via stdout       |
       |                           |<----------------------------|
       |                           |                             |
       |                           |  Resolve pending approval   |
       |                           |  by request_id              |
       |                           |                             |
       |  Tool execution proceeds  |                             |
       |  or is cancelled          |                             |
```

---

## 6. Auto-Routing Behavior

When a channel message is injected without specifying a session, LibrAgent uses **auto-routing** to find the target:

| Matching Sessions | Behavior                                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **0 matches**     | Error: `"No active session is currently connected to channel server '{server_name}'"`                                                                  |
| **1 match**       | Message injected into that session                                                                                                                     |
| **2+ matches**    | Error: `"Ambiguous active sessions for channel server '{server_name}': {list}. Use the session-scoped channel endpoint to target a specific session."` |

**Matching criteria:** The session must have an active connection to the named MCP server through the session-isolated proxy layer.

**For unambiguous cases**, use the sessionless endpoints (Section 7.1). For multi-session scenarios, use the session-scoped endpoint (Section 7.2).

---

## 7. Integration Endpoints

### 7.1 Bridge: Sessionless Ingress (Production)

**Tauri command:**

```typescript
// From frontend or external bridge
await invoke('agent_inject_channel_message_auto', {
  serverName: 'telegram',
  content: 'Hello from Telegram!',
  meta: { chat_id: '12345', sender_name: 'Alice' },
});
```

**HTTP API:**

```http
POST /api/channel
Content-Type: application/json

{
  "serverName": "telegram",
  "content": "Hello from Telegram!",
  "meta": { "chat_id": "12345", "sender_name": "Alice" }
}
```

**Response:**

```json
{
  "messageId": "uuid-here",
  "processed": true
}
```

| Response field     | Meaning                                      |
| ------------------ | -------------------------------------------- |
| `processed: true`  | Message injected; workflow woken immediately |
| `processed: false` | Message queued while session is busy         |

### 7.2 Bridge: Session-Scoped Ingress

**Tauri command:**

```typescript
await invoke('agent_inject_channel_message', {
  sessionId: 'abc123...',
  serverName: 'telegram',
  content: 'Hello from Telegram!',
  meta: { chat_id: '12345' },
});
```

**HTTP API:**

```http
POST /api/sessions/{sessionId}/channel
Content-Type: application/json

{
  "serverName": "telegram",
  "content": "Hello from Telegram!",
  "meta": { "chat_id": "12345" }
}
```

### 7.3 Bridge: Permission Response

**Tauri command:**

```typescript
await invoke('agent_respond_channel_permission', {
  sessionId: 'abc123...',
  requestId: 'a1b2c3d4...',
  behavior: 'allow',
});
```

**HTTP API:**

```http
POST /api/sessions/{sessionId}/channel/permission
Content-Type: application/json

{
  "requestId": "a1b2c3d4...",
  "behavior": "allow"
}
```

---

## 8. Drop Metrics & Monitoring

LibrAgent tracks dropped channel events for operational monitoring:

| Metric            | Description                                           |
| ----------------- | ----------------------------------------------------- |
| `buffer_full`     | Events dropped because the 1,024-event buffer is full |
| `receiver_closed` | Events dropped because the session receiver is closed |

**Logging:** Dropped events are aggregated and logged as `warn!` every 60 seconds:

```
Channel event drops in last 60s (triggered by 'telegram'): buffer_full=5, receiver_closed=0
```

**Recommendation:** Monitor these metrics. Persistent `buffer_full` drops indicate the agent is too slow to process incoming channel messages.

---

## 9. Example Implementations

### 9.1 Python (using `mcp` SDK)

```python
import asyncio
import json
import sys
from mcp.server.lowlevel import Server
from mcp.server.stdio import stdio_server

app = Server("my-channel-server")

@app.initialize
async def initialize():
    return {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "serverInfo": {
            "name": "my-channel-server",
            "version": "1.0.0",
            "instructions": "A sample channel server that pushes messages."
        },
        "experimental": {
            "claude/channel": {},
            "claude/channel/permission": {}
        }
    }

async def send_channel_message(content: str, meta: dict | None = None):
    """Send a channel message to LibrAgent via stdout."""
    notification = {
        "jsonrpc": "2.0",
        "method": "claude/channel",
        "params": {
            "content": content,
            "meta": meta or {}
        }
    }
    # Write line-delimited JSON to stdout
    sys.stdout.write(json.dumps(notification) + "\n")
    sys.stdout.flush()

async def send_permission_verdict(request_id: str, behavior: str):
    """Send a permission verdict to LibrAgent via stdout."""
    verdict = {
        "jsonrpc": "2.0",
        "method": "claude/channel/permission",
        "params": {
            "request_id": request_id,
            "behavior": behavior  # "allow" or "deny"
        }
    }
    sys.stdout.write(json.dumps(verdict) + "\n")
    sys.stdout.flush()

async def main():
    async with stdio_server() as (read_stream, write_stream):
        # ... handle MCP protocol messages ...
        # When you need to push a message:
        await send_channel_message(
            "Hello from Python!",
            {"chat_id": "12345", "sender_name": "Bot"}
        )

asyncio.run(main())
```

### 9.2 Node.js (using `@modelcontextprotocol/sdk`)

```typescript
import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  InitializeRequestSchema,
  NotificationSchema,
} from '@modelcontextprotocol/sdk/types.js';

const server = new Server(
  { name: 'my-channel-server', version: '1.0.0' },
  {
    capabilities: {
      experimental: {
        'claude/channel': {},
        'claude/channel/permission': {},
      },
    },
  },
);

// Inject channel capability into initialize response
server.setRequestHandler(InitializeRequestSchema, async () => {
  return {
    protocolVersion: '2024-11-05',
    capabilities: {},
    serverInfo: {
      name: 'my-channel-server',
      version: '1.0.0',
      instructions: 'A sample channel server.',
    },
    experimental: {
      'claude/channel': {},
      'claude/channel/permission': {},
    },
  };
});

function sendChannelMessage(content: string, meta?: Record<string, string>) {
  const notification = {
    jsonrpc: '2.0' as const,
    method: 'claude/channel',
    params: { content, meta: meta ?? {} },
  };
  process.stdout.write(JSON.stringify(notification) + '\n');
}

function sendPermissionVerdict(requestId: string, behavior: 'allow' | 'deny') {
  const verdict = {
    jsonrpc: '2.0' as const,
    method: 'claude/channel/permission',
    params: { request_id: requestId, behavior },
  };
  process.stdout.write(JSON.stringify(verdict) + '\n');
}

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  // ... handle MCP messages ...
  // Push a message:
  sendChannelMessage('Hello from Node!', { chat_id: '12345' });
}

main();
```

### 9.3 Raw Stdio (Language-Agnostic)

Any process can act as a LibrAgent channel server by:

1. Being spawned by LibrAgent with stdin/stdout pipes
2. Responding to the MCP `initialize` request with channel capabilities
3. Writing JSON-RPC notifications to stdout (one per line)

```bash
# Example: a shell script as a minimal channel server
echo '{"jsonrpc":"2.0","result":{"protocolVersion":"2024-11-05","capabilities":{},"serverInfo":{"name":"echo-server","version":"1.0.0"},"experimental":{"claude/channel":{}}}}' >&2
echo '{"jsonrpc":"2.0","method":"claude/channel","params":{"content":"Hello from shell!","meta":{"via":"bash"}}}'
```

---

## 10. Security Considerations

### 10.1 Attribute Injection Prevention

LibrAgent blocks all HTML event handler attributes (`on*`) and the `style` attribute to prevent XSS when channel content renders in an HTML context. Meta keys that fail validation are **fallbacked** to the `[channel_meta]` body block instead of being silently dropped.

**Safe meta keys for attributes:** `chat_id`, `sender_name`, `timestamp`, `priority`, `_internal_flag`

**Blocked meta keys (as attributes):** `onclick`, `onerror`, `style`, `source`

**Fallback keys (as body):** `oncall`, `one`, `online`, `style`, `SOURCE`

### 10.2 XML Escaping

All content and attribute values are XML-escaped. The `>` character is escaped in both contexts, while `"` is only escaped in attribute values. This prevents XML structure injection.

### 10.3 Content Size Limits

Content exceeding 8,192 bytes is silently dropped. This prevents buffer exhaustion attacks from malicious MCP servers.

### 10.4 Prompt Injection

**⚠️ Current limitation:** Channel messages are injected as `role: "user"` messages with **no content filtering**. A malicious MCP server can inject arbitrary instructions that the agent may follow. This is a known gap that requires a separate mitigation (e.g., content classification, role separation, or system prompt hardening).

---

## 11. Current Implementation Status

| Feature                            | Status | Notes                                       |
| ---------------------------------- | ------ | ------------------------------------------- |
| Server capability discovery        | ✅     | `experimental['claude/channel']`            |
| Channel message formatting         | ✅     | XML payload with safety filtering           |
| Bridge: sessionless ingress        | ✅     | Tauri + HTTP endpoints                      |
| Bridge: session-scoped ingress     | ✅     | Tauri + HTTP endpoints                      |
| Bridge: permission relay           | ✅     | Event + Tauri/HTTP response                 |
| Frontend channel rendering         | ✅     | Distinct UI for channel messages            |
| Native stdio transport             | ✅     | `ChannelAwareStdioTransport`                |
| Native `claude/channel` parsing    | ✅     | `ChannelInterceptCodec`                     |
| Native `claude/channel/permission` | ✅     | Via `channel_transport`                     |
| Permission request broadcast       | ✅     | To servers with `claude/channel/permission` |
| Auto-routing                       | ✅     | 1-session unambiguous routing               |
| Drop metrics logging               | ✅     | 60s interval, atomic counters               |
| Prompt injection mitigation        | ❌     | Known gap, separate fix needed              |

---

## 12. Troubleshooting

| Symptom                            | Likely Cause                                       | Fix                                               |
| ---------------------------------- | -------------------------------------------------- | ------------------------------------------------- |
| Messages not appearing in agent    | Server not advertising `claude/channel` capability | Add `"claude/channel": {}` to initialize response |
| Meta keys silently missing         | Key blocked by attribute filter                    | Use safe key names or accept body-block fallback  |
| Large messages dropped             | Content exceeds 8,192 bytes                        | Split into smaller messages                       |
| `buffer_full` warnings             | Agent too slow to process messages                 | Reduce message frequency or optimize agent        |
| Permission verdict ignored         | Wrong `request_id` format                          | Use UUID v4 (32-char lowercase hex)               |
| `Ambiguous active sessions` error  | Multiple sessions connected to same server         | Use session-scoped endpoint                       |
| Messages arrive as unreadable text | JSON not line-delimited or missing `\n`            | Ensure one JSON object per line with newline      |

---

## 13. Changelog

| Date       | Version | Changes                                                                                  |
| ---------- | ------- | ---------------------------------------------------------------------------------------- |
| Date       | Version | Changes                                                                                  |
| ---------- | ------- | ------------------------------------------------------------                             |
| 2026-06-27 | 0.1     | Initial reference guide with P1/P2 fixes included (UUID, attribute filter, drop metrics) |
