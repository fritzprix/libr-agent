# LibrAgent `claude/channel` Implementation Reference

> **For MCP server developers** who want to push messages into LibrAgent agent sessions via `claude/channel` notifications.

---

## 1. Overview

LibrAgent implements Anthropic's proprietary `claude/channel` protocol to allow external MCP servers to **push messages directly into agent sessions** as user input. When a channel message arrives, it is injected as a `role: "user"` message, which triggers the agent's workflow (idle → Queued → Busy → LLM completion).

### Protocol Methods

LibrAgent accepts **two method name variants** for each notification type (both work identically):

| Purpose            | Method 1 (canonical)        | Method 2 (compat)                         |
| ------------------ | --------------------------- | ----------------------------------------- |
| Push message       | `claude/channel`            | `notifications/claude/channel`            |
| Permission verdict | `claude/channel/permission` | `notifications/claude/channel/permission` |

Both variants are parsed by `normalize_channel_method()`. Use whichever your MCP SDK generates.

---

## 2. Push Message Notification

### JSON-RPC Format

```json
{
  "jsonrpc": "2.0",
  "method": "claude/channel",
  "params": {
    "content": "Hello from Telegram!",
    "meta": {
      "chat_id": "12345",
      "sender_id": "42",
      "sender_name": "John"
    }
  }
}
```

### Parameters

| Field     | Type     | Required | Description                                                                               |
| --------- | -------- | -------- | ----------------------------------------------------------------------------------------- |
| `content` | `string` | **Yes**  | The message body. Sent to the LLM as user text.                                           |
| `meta`    | `object` | No       | Key-value metadata. Values are **always converted to strings** (numbers, booleans, null). |

### Meta Value Conversion

All meta values are flattened to strings via `meta_value_to_string()`:

```
"sender_id": 42           → "42"
"is_admin": true          → "true"
"null_value": null        → "" (empty string)
"text": "hello"           → "hello"
```

### Content Limits

| Constraint      | Value                        | Behavior on violation                                             |
| --------------- | ---------------------------- | ----------------------------------------------------------------- |
| Max bytes       | **8,192** (8 KB)             | **Silently dropped** — no error returned, event discarded         |
| Buffer capacity | **1,024** events per session | When full, new events are **silently dropped** with a `warn!` log |

### Event Routing

- Channel events are **session-scoped**. Each LibrAgent session has its own bounded `mpsc::channel(1024)`.
- The dispatch task (`spawn_session_channel_dispatch_task`) reads from the session's event receiver and routes to `inject_channel_notification()`.
- If the `AgentSessionManager` is unavailable (e.g., app still initializing), events are **dropped with a warning**.

---

## 3. Message Format in Session

When a channel notification is received, LibrAgent transforms it into a `Message` and injects it into the session's conversation history.

### XML Wrapping

The raw `content` string is wrapped in an XML element with attributes derived from `server_name` and `meta`:

```xml
<source="telegram" chat_id="12345" sender_id="42" sender_name="John">
Hello from Telegram!
</source>
```

### Meta Attribute Rules

LibrAgent validates meta key names with `is_safe_channel_attribute_name()`:

**Allowed characters**: ASCII alphabetic (first char), then alphanumeric + `_` + `-`  
**Reserved**: `source` is always rejected (reserved for server name)

| Key           | Valid? | Reason                          |
| ------------- | ------ | ------------------------------- |
| `chat_id`     | ✅     | Alphanumeric + underscore       |
| `sender-name` | ✅     | Alphanumeric + hyphen           |
| `123invalid`  | ❌     | First char must be alpha or `_` |
| `chat id`     | ❌     | Space not allowed               |
| `source`      | ❌     | Reserved for server name        |

### Invalid Meta Fallback

If a meta key fails validation, it is **not** added as an attribute. Instead, it is appended as text content under a `[channel_meta]` block:

```xml
<source="telegram" chat_id="12345">
Hello from Telegram!

[channel_meta]
123invalid=bad_key_value
chat id=space_not_allowed
[/channel_meta]
</source>
```

### XML Escaping

| Character | Escaped to | Context                  |
| --------- | ---------- | ------------------------ |
| `&`       | `&amp;`    | Both attributes and text |
| `"`       | `&quot;`   | Attributes only          |
| `<`       | `&lt;`     | Both attributes and text |
| `>`       | `&gt;`     | Both attributes and text |

**Note**: `]]>` and `<![` sequences are **not** escaped. Safe for current usage (text content), but do not embed raw CDATA markers in `content`.

---

## 4. Workflow Behavior

### When Session is Idle

```
Channel message → inject_messages() → should_trigger_workflow = true
  → Message persisted to DB
  → Execution state reset
  → Status: Idle → Queued → Busy
  → WorkflowStarted event emitted
  → LLM completion triggered (request_llm_completion_with_recovery)
```

The agent will pick up the injected message and respond.

### When Session is Already Busy or Queued

```
Channel message → inject_messages() → should_trigger_workflow = false
  → Message persisted to DB (stored, not dropped)
  → No status transition, no LLM trigger
  → Message waits in queue until session becomes idle
```

The message is **not lost** — it sits in the database and will be processed when the session becomes idle again. However, there is **no UI notification** to the user that a message was queued.

### Background Task Flow

When `should_trigger_workflow = true`, a background task is spawned that:

1. Acquires a concurrency gate permit (`gate.acquire_active_agent()`)
2. Double-checks session is not cancelled and status is still Queued
3. Sets `session.active_permit`
4. Transitions status to Busy
5. Ensures MCP proxy is ready (60s timeout)
6. Calls `request_llm_completion_with_recovery()`
7. On error: sets status to Error and emits `WorkflowError` event

---

## 5. Permission Request Flow

When an agent needs user permission for a tool call triggered by a channel message, LibrAgent creates a pending approval with a `request_id`. The MCP server (or external system) resolves it via a channel notification.

### Permission Request (from Agent)

The agent creates a pending tool approval with `request_id`. This is internal to LibrAgent — you don't need to generate it.

### Permission Verdict (from Server)

```json
{
  "jsonrpc": "2.0",
  "method": "claude/channel/permission",
  "params": {
    "request_id": "abc123def456",
    "behavior": "allow"
  }
}
```

### Parameters

| Field        | Type     | Required | Description                                       |
| ------------ | -------- | -------- | ------------------------------------------------- |
| `request_id` | `string` | **Yes**  | Matches the `request_id` on the pending approval. |
| `behavior`   | `string` | **Yes**  | Must be `"allow"` or `"deny"`.                    |

### Behavior Values

| Value         | Result                                            |
| ------------- | ------------------------------------------------- |
| `"allow"`     | Tool execution approved                           |
| `"deny"`      | Tool execution denied                             |
| anything else | **Default to deny** (no longer hangs the session) |

### Resolution Flow

```
Permission verdict notification → parse_channel_permission_behavior()
  → find_pending_approval_tool_call_id(request_id)
  → respond_tool_approval(tool_call_id, approved)
    → removes from pending_approvals map
    → sends verdict via oneshot channel to the waiting LLM
```

If the `request_id` is not found in pending approvals, the verdict is **silently dropped** with a warning.

---

## 6. Multi-Session Routing

LibrAgent supports multiple sessions, each with potentially different MCP servers. Channel notifications are routed to the correct session automatically.

### Auto-Routing Logic

When a channel notification arrives, LibrAgent:

1. Finds all active sessions that have the notifying server attached (`session_has_channel_server()`)
2. If exactly one match → routes to that session
3. If multiple matches → uses `resolve_auto_routed_channel_target()` (policy-based, e.g., most recent, parent-based)
4. If no matches → event is dropped

### Server Attachment

A session "has" a server when the server was configured as part of that session's MCP toolset. The routing check is:

```rust
manager.proxy_manager.session_has_channel_server(&session_id, server_name).await
```

---

## 7. Example: Telegram Server Push

```typescript
// Pseudocode for an MCP server that receives Telegram messages
// and pushes them into LibrAgent

async function onTelegramMessage(message: TelegramMessage) {
  // 1. Build the channel notification
  const notification = {
    jsonrpc: '2.0',
    method: 'claude/channel',
    params: {
      content: message.text,
      meta: {
        chat_id: String(message.chat.id),
        sender_id: String(message.from.id),
        sender_name: message.from.first_name || '',
        timestamp: String(message.date),
      },
    },
  };

  // 2. Send as a notification (no id field — must be a notification, not a request)
  await server.notification({
    method: notification.method,
    params: notification.params,
  });

  // 3. If the agent responds with a permission request,
  //    wait for user decision, then send verdict:
  //
  // await server.notification({
  //   method: "claude/channel/permission",
  //   params: {
  //     request_id: pendingRequest.requestId,
  //     behavior: "allow", // or "deny"
  //   },
  // });
}
```

---

## 8. Gotchas & Best Practices

### ✅ Do

- **Keep `content` under 8 KB** — larger messages are silently dropped
- **Use string-safe meta keys** — only `[a-zA-Z_][a-zA-Z0-9_-]*`
- **Send notifications without an `id` field** — requests with `id` are ignored by `try_parse_channel_event()`
- **Use `"allow"` or `"deny"` for permission verdicts** — other values default to deny but waste a round-trip
- **Handle buffer overflow gracefully** — if your server sends faster than 1024 events/session, expect drops

### ❌ Don't

- **Don't embed `]]>` in content** — not escaped, could confuse downstream XML parsers
- **Don't rely on queue notifications** — messages to busy sessions are persisted but not surfaced to UI
- **Don't assume session exists** — if the session is deleted or the app is initializing, events are dropped
- **Don't send too many events too fast** — the 1024-element buffer has no backpressure; excess events are dropped

### 🔍 Debugging Tips

- Check backend logs for `"Channel event buffer full"` — indicates your server is flooding the buffer
- Check for `"Dropping channel event"` — means the dispatch task can't find the AgentSessionManager
- Check for `"Failed to deliver native channel notification"` — means the session was not found during injection
- Check for `"Invalid channel permission verdict"` — means your `behavior` value was not `"allow"` or `"deny"`

---

## 9. Implementation Reference

| Component        | File                                         | Key Function                                                                           |
| ---------------- | -------------------------------------------- | -------------------------------------------------------------------------------------- |
| Parsing          | `mcp/session_isolation/channel_events.rs`    | `try_parse_channel_event()`, `normalize_channel_method()`                              |
| Dispatch         | `mcp/session_isolation/channel_dispatch.rs`  | `spawn_session_channel_dispatch_task()`                                                |
| Message Building | `agent/session_manager/channel.rs`           | `inject_channel_notification()`, `build_channel_message()`, `format_channel_payload()` |
| Injection        | `agent/session_manager/message_injection.rs` | `inject_messages()`                                                                    |
| Permission       | `agent/session_manager/approvals.rs`         | `respond_channel_permission()`                                                         |
| Approval Lookup  | `agent/tool_approvals.rs`                    | `parse_channel_permission_behavior()`, `find_pending_approval_tool_call_id()`          |

---

## 10. Protocol Comparison: `claude/channel` vs `notifications/message`

|                           | `claude/channel`                         | `notifications/message` |
| ------------------------- | ---------------------------------------- | ----------------------- |
| **Purpose**               | Interactive push messages                | Logging/notification    |
| **Becomes user message?** | ✅ Yes, triggers workflow                | ❌ No, log-only         |
| **Session-scoped**        | ✅ Yes, routed to specific session       | ❌ No, global           |
| **Permission flow**       | ✅ `claude/channel/permission`           | ❌ None                 |
| **Metadata**              | ✅ `meta` object → XML attributes        | ❌ Opaque `data` field  |
| **LibrAgent support**     | ✅ Full (push, inject, workflow trigger) | ✅ Log-only             |

Use `claude/channel` when you want the MCP server to **converse** with the agent. Use `notifications/message` when you just want to **log** something.
