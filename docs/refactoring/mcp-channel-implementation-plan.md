# Implementation Plan: MCP Channel Support

This plan details the code-level changes required to implement Channel support in LibrAgent.

## Phase 1: Core Protocol & Types

### 1.1 MCP Types Extension

**File:** `src-tauri/src/mcp/types.rs`

- Update `ServerCapabilities` struct:
  ```rust
  pub struct ServerCapabilities {
      pub tools: Option<serde_json::Value>,
      pub resources: Option<serde_json::Value>,
      pub prompts: Option<serde_json::Value>,
      // Add this:
      #[serde(skip_serializing_if = "Option::is_none")]
      pub experimental: Option<serde_json::Value>,
  }
  ```
- Update `InitializeResult` (within `MCPResponseResult::Initialize`) to include the top-level `instructions` field if present in the MCP 2025-06-18 spec or as an experimental extension.

### 1.2 Notification Models

**File:** `src-tauri/src/mcp/types.rs`

- Add structs for `ChannelNotification` and `PermissionRequest` to facilitate type-safe serialization/deserialization.

---

## Phase 2: Transport Layer (Async Notifications)

### 2.1 Stdio Manager Extension

**File:** `src-tauri/src/mcp/session_isolation/stdio_manager/process.rs` (or related)

- Current: The manager mostly handles request-response pairs, but `rmcp` exposes initialize metadata through `peer_info()`.
- Immediate change: Capture `experimental` capabilities and `instructions` after the initialize handshake and cache them per server.
- Deferred change: true arbitrary custom notification support (`notifications/claude/channel`) requires either upstream `rmcp` support or a lower-level transport bridge because `ServerNotification` is strongly typed in `rmcp` 0.8.x.

### 2.2 HTTP/SSE Manager Extension

**File:** `src-tauri/src/mcp/session_isolation/http_manager.rs`

- Mirror the same initialize-metadata capture and channel capability caching as stdio connections.

---

## Phase 3: Agent Orchestration

### 3.1 Session Manager & Bus

**File:** `src-tauri/src/agent/session_manager.rs`

- Add an explicit channel injection path that accepts `(session_id, server_name, content, meta)`.
- Wrap the payload in a `<channel>` XML tag and append it to the current conversation as a user-role message with `source = "channel"`.
- **Interrupt Logic**: If the agent is idle, trigger a new `Think` phase; if it is busy, queue the channel message in `pending_events`.

### 3.2 System Prompt Builder

**File:** `src-tauri/src/agent/session_manager.rs`

- Modify `build_system_prompt` to iterate through all active `MCPServiceProxy` servers.
- If a server has `experimental.claude/channel`, extract its `instructions` and add them to the system prompt.

---

## Phase 4: Permission Relay

### 4.1 Approval Interceptor

**File:** `src-tauri/src/agent/tool_approvals.rs`

- Update the approval flow:
  1. When a tool requires approval, broadcast a `permission_request` notification to all capable channels.
  2. Maintain a map of `request_id -> oneshot::Sender`.
  3. Listen for `notifications/claude/channel/permission` from any channel.
  4. The first to arrive (Local UI or Channel) completes the oneshot and proceeds.

---

## Phase 5: Frontend Integration

### 5.1 UI Components

**File:** `src/features/chat/components/MessageItem.tsx`

- Add rendering logic for messages with a "channel" origin.
- Display metadata (attributes) as small badges or labels.

### 5.2 Settings & Configuration

**File:** `src/features/assistants/components/AssistantForm.tsx`

- Add a toggle/section to enable/disable specific channel servers for an assistant.
