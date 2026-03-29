# Specification: MCP Channel Protocol Support

This document outlines the implementation plan for supporting "Channel" style MCP servers in LibrAgent, following the protocol established by Claude Code.

## 1. Overview

"Channels" are MCP servers that push real-time events (webhooks, chat messages, alerts) into an AI agent session. Unlike standard MCP tools which are reactive (AI calls them), Channels are proactive (they notify the AI).

### Core Features
- **Inbound Events**: External systems push text/metadata to the agent via MCP notifications.
- **Instruction Injection**: Channels provide specific system prompt snippets to guide the agent on how to handle their events.
- **Two-Way Communication**: Using standard MCP tools (e.g., `reply`) to send messages back.
- **Permission Relay**: Off-device approval for sensitive tool executions.

---

## 2. Protocol Extensions

LibrAgent will adopt the following `experimental` MCP capabilities and methods:

### Capabilities (Discovery)
- `capabilities.experimental['claude/channel']`: Marks the server as a channel.
- `capabilities.experimental['claude/channel/permission']`: Opts in to remote permission relay.
- `instructions`: (Field in `InitializeResult`) A string to be appended to the agent's system prompt.

### Notifications (Server -> Client)
- **`notifications/claude/channel`**:
  - `params.content`: The message body.
  - `params.meta`: Map of attributes (e.g., `chat_id`, `sender`).
- **`notifications/claude/channel/permission`**:
  - `params.request_id`: The ID of the pending request.
  - `params.behavior`: `"allow"` | `"deny"`.

### Notifications (Client -> Server)
- **`notifications/claude/channel/permission_request`**:
  - `params.request_id`: Unique 5-letter ID.
  - `params.tool_name`: Tool being requested.
  - `params.description`: Human-readable summary.
  - `params.input_preview`: Truncated JSON arguments.

---

## 3. Architecture Implementation (Rust Backend)

### 3.1 Metadata & Schema (`src-tauri/src/mcp/types.rs`)
Update `ServerCapabilities` and `InitializeResult` to parse the `experimental` field and the top-level `instructions`.

### 3.2 Notification Listener (`src-tauri/src/mcp/session_isolation/`)
Capture Channel server metadata (`experimental`, `instructions`) from the MCP initialize handshake and persist it in the session-isolated managers.
- `rmcp` currently exposes initialize metadata cleanly via `peer_info()`.
- `rmcp` 0.8.x does **not** provide a drop-in path for arbitrary custom server notifications such as `notifications/claude/channel` because server notifications are strongly typed in the client wrapper.
- For now, inbound channel messages should enter LibrAgent through an explicit internal bridge (e.g. HTTP/Tauri command) that injects a channel-formatted message into the target session.

### 3.3 System Prompt Injection (`src-tauri/src/agent/session_manager.rs`)
Update `build_system_prompt` to:
1. Collect all `instructions` from registered Channel servers in the session.
2. Append them as a dedicated "## Channels" section in the system prompt.

### 3.4 Context Injection (`src-tauri/src/agent/workflow/`)
When an inbound channel message arrives through the bridge:
1. Format the payload into an XML-like tag:
   ```xml
   <channel source="server_name" attr1="val1" ...>
   content
   </channel>
   ```
2. Insert this as a new `Message` (role: `user` or a custom `channel` role) into the active conversation.
3. Trigger the agent loop if it is currently idling.

### 3.5 Permission Relay (`src-tauri/src/agent/tool_approvals.rs`)
When a tool requires approval:
1. Check if any active channel has the `claude/channel/permission` capability.
2. If yes, generate a `request_id` and send `permission_request` to those channels.
3. The first response (Local UI or Remote Channel) wins.

---

## 4. Implementation Phases

### Phase 1: Foundation & Inbound (P0)
- Update MCP types and capability discovery.
- Capture channel metadata during MCP initialization.
- Support `instructions` injection into system prompts.
- Implement basic `<channel>` tag injection into conversation history and wake idle workflows.

### Phase 2: Two-Way & UI (P1)
- Support standard tool calls (`reply`) within the channel context.
- Add UI indicators in the LibrAgent chat interface showing that a message came from an external channel.
- Manage "Channel Access" in the Assistant configuration UI.

### Phase 3: Permission Relay (P2)
- Implement the `permission_request` broadcast logic.
- Implement the `permission` response handling to resolve pending tool approvals.

---

## 5. Security Considerations

- **Prompt Injection**: Channel messages must be clearly demarcated to prevent the agent from confusing channel input with direct user instructions.
- **Sender Validation**: Documentation should emphasize that channel servers must perform their own auth (Sender Gating).
- **Session Isolation**: Since LibrAgent uses session-isolated MCP proxies, channel events must never leak across session boundaries.
