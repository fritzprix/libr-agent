# Claude Channels Implementation Status

This document outlines the current implementation state of the Claude Channels (proactive notification) extension in LibrAgent and identifies the remaining tasks required for full automation.

## Overview

Claude Channels allows external MCP servers to proactively inject messages into an active agent session and request tool execution permissions. In LibrAgent, this is designed to bridge external events (e.g., a file change, a webhook, or a long-running task completion) directly into the agent's reasoning loop.

## 1. Completed Features ✅

### 1.1 Data Models & Protocol Extensions
- **Location:** `src-tauri/src/mcp/types.rs`
- **Implemented:** `ChannelNotification`, `ChannelPermissionRequest`, `ChannelPermissionVerdict`, and `ChannelServerMetadata` structs are defined to match the proposed protocol.

### 1.2 Automated Discovery
- **Location:** `src-tauri/src/mcp/session_isolation/channel_metadata.rs`
- **Implemented:** Automatic extraction of channel capabilities from the `initialize` response of external servers. It detects `claude/channel` and `claude/channel/permission` support.

### 1.3 System Prompt Integration
- **Location:** `src-tauri/src/mcp/service_proxy/mod.rs`
- **Implemented:** `MCPServiceProxy` automatically injects a "## Channels" section into the system prompt, informing the AI agent which connected servers have proactive messaging capabilities and providing any server-specific instructions.

### 1.4 Core Injection Engine
- **Location:** `src-tauri/src/agent/session_manager.rs`
- **Implemented:** `AgentSessionManager::inject_channel_notification` provides the logic to:
    1. Wrap channel content in a specialized `<channel>` XML tag.
    2. Persist it as a "user" role message with `source: "channel"`.
    3. Trigger the agent's workflow if the session is currently idle.

### 1.5 External Control Points
- **Implemented:** 
    - Tauri Command: `agent_inject_channel_message`
    - HTTP API: `POST /api/sessions/{id}/channel`
    - These allow external processes to manually trigger the injection engine.

## 2. Missing Features & Gaps ❌

### 2.1 Automated Notification Handling (Critical Gap)
- **Current State:** Both `SessionMCPManager` (stdio) and `HttpSessionManager` (HTTP) use `().serve(transport)`.
- **Issue:** The empty tuple `()` provides no implementation for the `rmcp::service::Handler` trait. As a result, when an MCP server sends a `notifications/claude/channel` JSON-RPC call, it is silently ignored by the client.
- **Requirement:** A dedicated `ChannelNotificationHandler` must be implemented and passed to the `serve()` method during server startup.

### 2.2 Permission Relay Workflow
- **Issue:** There is no logic to handle `notifications/claude/channel/permission_request`. 
- **Requirement:** A UI flow to show the request to the user and a mechanism to send the `notifications/claude/channel/permission` response back to the server.

### 2.3 Frontend Visual Distinction
- **Issue:** Channel-injected messages appear as standard user messages.
- **Requirement:** The `AgentMessageBubble` component should detect the `source: "channel"` or `metadata.channel` field and render a distinct "Notification" or "Channel" badge to prevent user confusion.

## 3. Recommended Implementation Path

### Phase 1: Real-time Handler
1. Create `src-tauri/src/mcp/session_isolation/notification_handler.rs`.
2. Implement `rmcp::service::Handler` for a new `McpSessionHandler` struct.
3. Use `AppHandle` to lookup the `AgentSessionManager` and call `inject_channel_notification` when a notification arrives.

### Phase 2: Permission UI
1. Define a new `AgentEvent` for permission requests.
2. Implement a `PermissionRequestWidget` in the frontend.
3. Add a backend service to relay the user's verdict back to the specific MCP server connection.

### Phase 3: UI Polish
1. Update `AgentMessageBubble.tsx` to render a specialized header for messages where `source === 'channel'`.
2. Include the `server_name` in the bubble UI to clearly identify which external tool sent the message.

---
*Last Updated: 2026-03-29*
