# Specification: Claude Channels Support in LibrAgent

This document tracks the official Claude Channels protocol and how LibrAgent maps that protocol onto its current architecture.

## 1. Official Claude Channels Model

According to the Claude Code documentation:

- A **channel** is an MCP server that runs locally and pushes events into an already-open session.
- Channel servers connect over **stdio** and declare `capabilities.experimental['claude/channel']`.
- Two-way channels also expose standard MCP tools (for example a `reply` tool).
- Channels that support remote approvals additionally declare `capabilities.experimental['claude/channel/permission']`.
- Channel-specific `instructions` are appended to the model's system prompt.

### Official notification flow

#### Server -> client

- `notifications/claude/channel`
  - `params.content: string`
  - `params.meta?: Record<string, string>`
- `notifications/claude/channel/permission`
  - `params.request_id: string`
  - `params.behavior: 'allow' | 'deny'`

#### Client -> server

- `notifications/claude/channel/permission_request`
  - `params.request_id: string`
  - `params.tool_name: string`
  - `params.description: string`
  - `params.input_preview: string`

### Official payload semantics

- `meta` keys become attributes on the injected `<channel>` tag.
- The `source` attribute is derived from the MCP server name.
- Channels must enforce **sender gating / allowlists** before forwarding anything to the agent.
- Permission relay is optional and only applies to tool-use approval prompts, not all trust dialogs.

## 2. LibrAgent Design Decision

LibrAgent follows the Claude protocol shape where practical, but the current runtime is split into two layers:

1. **Protocol compatibility**
   - parse channel capabilities and `instructions`
   - preserve channel-shaped payloads and permission-relay types
   - inject channel content as `<channel ...>` messages into the active agent session

2. **Current transport reality**
   - channel metadata is discovered from MCP initialize responses
   - inbound channel events currently enter through an explicit LibrAgent bridge
   - native handling of arbitrary custom MCP notifications is not yet wired into the rmcp client path used by LibrAgent session managers

That means LibrAgent is currently **bridge-compatible** with Claude-style channels, but not yet a full drop-in implementation of direct `notifications/claude/channel*` delivery from connected MCP servers.

## 3. Current Architecture Mapping

### 3.1 Metadata discovery

Implemented in session-isolated MCP managers:

- detect `experimental['claude/channel']`
- detect `experimental['claude/channel/permission']`
- capture top-level `instructions`

### 3.2 Prompt integration

Registered channel servers contribute a dedicated `## Channels` section to the agent system prompt. This gives the model channel-specific behavioral guidance without requiring a tool call first.

### 3.3 Inbound bridge path

Current LibrAgent ingress points:

- Tauri command: `agent_inject_channel_message`
- Tauri command: `agent_inject_channel_message_auto`
- HTTP route: `POST /api/channel`
- HTTP route: `POST /api/sessions/{id}/channel`

Those paths:

1. build a `<channel source="..." ...>` payload
2. persist it as a session message with `role: "user"` and `source: "channel"`
3. wake the workflow when the target session is idle

#### Supported bridge request/response contract

**Tauri command**

- `agent_inject_channel_message`
  - request:
    - `sessionId: string`
    - `serverName: string`
    - `content: string`
    - `meta?: Record<string, string>`
  - response:
    - `success: true`
    - `data.status: 'processed' | 'queued'`

**HTTP**

- `POST /api/sessions/{id}/channel`
  - body:
    - `serverName: string`
    - `content: string`
    - `meta?: Record<string, string>`
  - response:
    - `id: string`
    - `status: 'processed' | 'queued'`

`processed` means the session was idle and the workflow was woken immediately. `queued` means the message was persisted while the session was already busy.

#### Sessionless auto-routing contract

For bridges that do not have a meaningful LibrAgent `sessionId`, LibrAgent also supports **sessionless channel ingress**:

**Tauri command**

- `agent_inject_channel_message_auto`
  - request:
    - `serverName: string`
    - `content: string`
    - `meta?: Record<string, string>`
  - response:
    - `success: true`
    - `data.sessionId: string`
    - `data.sessionName: string`
    - `data.status: 'processed' | 'queued'`

**HTTP**

- `POST /api/channel`
  - body:
    - `serverName: string`
    - `content: string`
    - `meta?: Record<string, string>`
  - response:
    - `id: string`
    - `sessionId: string`
    - `sessionName: string`
    - `status: 'processed' | 'queued'`

Auto-routing rule:

- if exactly one active session is currently connected to the given channel server, LibrAgent injects there
- if zero matching active sessions exist, the request fails
- if multiple matching active sessions exist, the request fails and the bridge must use the explicit session-scoped endpoint

This keeps the external bridge API ergonomic without silently delivering channel traffic into the wrong active session.

### 3.4 Native MCP notification gap

The current session-isolated stdio and HTTP managers still connect MCP clients via `().serve(transport)`. That means LibrAgent does not yet install a custom notification handler for:

- `notifications/claude/channel`
- `notifications/claude/channel/permission`

As a result, the official direct-push transport path is not complete yet.

### 3.5 Bridge-first permission relay contract

LibrAgent now supports a **bridge-first** approval relay flow even though the native MCP notification path is still missing.

#### Runtime event emitted by LibrAgent

When a tool call needs approval and the session has at least one connected channel server advertising `claude/channel/permission`, LibrAgent emits:

- `agent:event`
  - `type: 'channelPermissionRequest'`
  - `sessionId: string`
  - `requestId: string`
  - `toolCallId: string`
  - `toolName: string`
  - `description: string`
  - `inputPreview: string`

Important:

- this event is **only emitted** when the session actually has a permission-capable channel server
- `requestId` is an opaque short identifier generated by LibrAgent; bridges should treat it as a stable lookup key, not derive meaning from it
- the current frontend receives this event but intentionally does not render a dedicated UI for it yet

#### Approval response contract

**Tauri command**

- `agent_respond_channel_permission`
  - request:
    - `sessionId: string`
    - `requestId: string`
    - `behavior: 'allow' | 'deny'`
  - response:
    - `success: true`
    - `data.toolCallId: string`
    - `data.approved: boolean`

**HTTP**

- `POST /api/sessions/{id}/channel/permission`
  - body:
    - `requestId: string`
    - `behavior: 'allow' | 'deny'`
  - response:
    - `requestId: string`
    - `toolCallId: string`
    - `approved: boolean`

On success, LibrAgent resolves the matching pending approval and emits:

- `agent:event`
  - `type: 'toolExecutionApprovalResolved'`
  - `sessionId: string`
  - `toolCallId: string`
  - `approved: boolean`

If the session does not exist or the `requestId` no longer maps to a pending approval, the bridge receives a `404`.

Approval relay intentionally remains **session-scoped** today. We do not auto-route approval responses by `requestId` alone because the current short Claude-style request IDs are not treated as globally unique across all sessions.

## 4. Implementation Priorities

### Phase 1: Bridge-aligned UX and correctness

- Keep protocol docs accurate.
- Preserve `source: "channel"` and `metadata.channel` end-to-end.
- Render channel-originated messages distinctly in the chat UI.

### Phase 2: Productize the bridge contract

- Treat the explicit bridge ingress as the supported production contract today.
- Document the exact request/response/event payloads so external channel bridges can be implemented without reverse-engineering the codebase.
- Add backend coverage around approval relay resolution and stale-request handling.

### Phase 3: Native realtime notification intake

- Investigate rmcp support for arbitrary custom server notifications in LibrAgent's client setup.
- If feasible, prototype native `notifications/claude/channel` intake behind a feature flag rather than replacing the bridge immediately.
- If rmcp still blocks this cleanly, keep the bridge as the product boundary instead of forcing a brittle transport hack.

### Phase 4: Permission relay hardening

- Current bridge-first implementation path:
  - generate a Claude-style `request_id` when a pending approval opens
  - emit a bridge-friendly runtime event containing `request_id`, `tool_name`, `description`, and `input_preview`
  - accept remote `allow` / `deny` responses through explicit LibrAgent bridge APIs
- Future native implementation path:
  - broadcast `notifications/claude/channel/permission_request` to channel-capable servers when transport support exists
  - accept `notifications/claude/channel/permission` verdicts directly from MCP servers
- Keep sender-gated channels only; remote approval is security-sensitive by design.

## 5. Security Requirements

- Channel messages must remain clearly delimited as `<channel>` content.
- Channel servers are responsible for sender authentication / allowlists before forwarding messages.
- Channel events must remain session-isolated and must never leak across agent sessions.
- Remote permission relay must only be enabled for trusted, authenticated channels.
- External bridges must authenticate senders before calling the LibrAgent HTTP or Tauri ingress points; LibrAgent currently trusts the bridge boundary rather than re-identifying the original sender itself.
