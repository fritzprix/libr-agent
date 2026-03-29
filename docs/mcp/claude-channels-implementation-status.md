# Claude Channels Implementation Status

This document records the **actual** Claude Channels state in LibrAgent as of the current codebase.

## Summary

LibrAgent currently supports the **bridge-based** parts of Claude Channels well:

- channel capability discovery
- channel instructions in the system prompt
- explicit channel-message injection into an active agent session
- persisted `source: "channel"` messages with channel metadata
- distinct UI rendering for channel-originated messages

LibrAgent does **not yet** support the full direct Claude-style runtime where connected MCP servers can freely push `notifications/claude/channel*` into the session manager through the current rmcp client path.

## 1. Implemented Today ✅

### 1.1 Protocol-aligned data models

- **Location:** `src-tauri/src/mcp/types.rs`
- **Implemented:** `ChannelNotification`, `ChannelPermissionRequest`, `ChannelPermissionVerdict`, and `ChannelServerMetadata`.

### 1.2 Channel capability discovery

- **Location:** `src-tauri/src/mcp/session_isolation/channel_metadata.rs`
- **Implemented:** LibrAgent reads MCP initialize metadata and detects:
  - `experimental['claude/channel']`
  - `experimental['claude/channel/permission']`
  - top-level `instructions`

### 1.3 System prompt integration

- **Location:** `src-tauri/src/mcp/service_proxy/mod.rs`
- **Implemented:** Channel-capable servers contribute a `## Channels` section to the agent system prompt.

### 1.4 Bridge-based inbound channel injection

- **Location:** `src-tauri/src/agent/session_manager.rs`
- **Implemented:** `inject_channel_notification`:
  1. formats inbound content as a `<channel ...>` block
  2. persists it as a `role: "user"` message with `source: "channel"`
  3. wakes the workflow when the session is idle

### 1.5 External ingress points

- **Implemented:**
  - Tauri command: `agent_inject_channel_message`
  - HTTP API: `POST /api/sessions/{id}/channel`

### 1.6 Frontend channel rendering

- **Implemented:**
  - frontend message typing now accepts `source: "channel"`
  - backend message deserialization preserves `metadata.channel`
  - `AgentMessageBubble` renders channel-originated messages with a distinct notification header and server badge

### 1.7 Bridge-based permission relay primitives

- **Implemented:**
  - each pending approval now gets a Claude-style short `request_id`
  - LibrAgent emits a `channelPermissionRequest` agent event with `request_id`, `tool_name`, `description`, and `input_preview`
  - that event is now gated by real session capability: it is only emitted when the session has at least one connected channel server advertising `claude/channel/permission`
  - remote bridges can resolve approvals through:
    - Tauri command: `agent_respond_channel_permission`
    - HTTP API: `POST /api/sessions/{id}/channel/permission`

### 1.8 Supported bridge contract

- **Inbound message bridge**
  - sessionless Tauri: `agent_inject_channel_message_auto`
  - sessionless HTTP: `POST /api/channel`
  - Tauri: `agent_inject_channel_message`
  - HTTP: `POST /api/sessions/{id}/channel`
  - input:
    - `serverName`
    - `content`
    - `meta`
  - result:
    - `processed` when the workflow is woken immediately
    - `queued` when the message is persisted while the session is already busy
  - auto-routing behavior:
    - exactly 1 matching active session -> inject there
    - 0 matches -> fail
    - 2+ matches -> fail with ambiguity
  - matching is based on the active session being connected to the named channel server through the session-isolated proxy layer

- **Approval relay bridge**
  - runtime event: `channelPermissionRequest`
  - Tauri response path: `agent_respond_channel_permission`
  - HTTP response path: `POST /api/sessions/{id}/channel/permission`
  - accepted behaviors:
    - `allow`
    - `deny`
  - still session-scoped on purpose; approval responses are **not** auto-routed by `requestId` alone

## 2. Gaps Still Open ❌

### 2.1 Native custom notification intake

- **Current state:** Session-isolated MCP clients still connect with `().serve(transport)`.
- **Impact:** the direct Claude protocol path for `notifications/claude/channel` and `notifications/claude/channel/permission` is not wired into LibrAgent yet.
- **Note:** this is not just missing glue code; it requires confirming what rmcp exposes for arbitrary custom server notifications in the current client setup.

### 2.2 Permission relay workflow

- **Current state:** the backend bridge primitives now exist, but there is still no end-to-end external channel bridge bundled in LibrAgent.
- **Missing pieces:**
  - direct MCP-native `notifications/claude/channel/permission_request`
  - direct MCP-native `notifications/claude/channel/permission`
  - packaged bridge integration that forwards the emitted approval event to a real channel transport

### 2.3 Channel management UX

- **Current state:** channels can be discovered and messages can be injected, but there is no dedicated assistant/session UI for channel-specific access management comparable to Claude Code's `--channels` workflow.

## 3. Practical Interpretation

Right now, LibrAgent is best described as:

- **compatible with Claude Channels metadata and payload shape**
- **usable through explicit bridge injection**
- **not yet a full drop-in runtime for direct Claude Code channel notifications**

That distinction matters. The docs should not imply that a connected MCP server can already push custom channel notifications into LibrAgent without additional transport work.

## 4. Recommended Direction

1. Treat the bridge contract as the supported product boundary **today**.
2. Document the external bridge contract clearly enough that a real channel adapter can be built without reading Rust internals.
3. Add focused backend tests around approval relay resolution and stale/missing `request_id` handling.
4. Only prototype native `notifications/claude/channel*` intake behind a feature flag after the bridge contract is stable.

That recommendation is deliberate. The current rmcp client path still uses `().serve(transport)` with typed client notifications, so native Claude custom-notification intake is not a small glue patch.

---

_Last Updated: 2026-03-29_
