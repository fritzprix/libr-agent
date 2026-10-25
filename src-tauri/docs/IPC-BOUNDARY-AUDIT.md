# IPC Boundary Audit Report

**Project**: LibrAgent  
**Date**: 2026-10-18  
**Scope**: Rust backend (`src-tauri/`) ↔ TypeScript frontend (`src/`) communication surface  
**Mechanism**: Tauri `invoke_handler` (request/response) + `window.emit()` / `app.listen()` (events)

---

## Executive Summary

LibrAgent's IPC boundary consists of **~87 Tauri commands** registered in the `generate_handler!` block, communicated through a single centralized `safeInvoke<T>()` wrapper, and **~15 named events** emitted by the backend. The boundary is structurally sound but contains **critical deserialization safety gaps** (silent `.unwrap_or(Value::Null)` fallbacks in 2 DTOs), **missing frontend event listeners** for migration progress and OAuth callbacks, and **no input validation layer** on command parameters.

### Severity Summary

| Severity | Count | Description |
|----------|-------|-------------|
| 🔴 Critical | 2 | `.unwrap_or(Value::Null)` silently discards corrupt JSON in DTO deserialization |
| 🟡 High | 5 | Missing frontend event listeners, `.unwrap()` on JSON serialization, password `unwrap_or("")` fallback |
| 🟠 Medium | 8 | Unchecked optional parameters, no Zod/schema validation, incomplete type mapping |
| 🟢 Low | 6 | Naming inconsistencies, redundant code paths, documentation gaps |

---

## 1. Command Inventory

All commands are registered in `src-tauri/src/lib.rs` `generate_handler!` block (lines 216–438).

### 1.1 Command Categories

| Category | Count | Examples |
|----------|-------|----------|
| **Agent Workflow** | 33 | `agent_create_session`, `agent_send_message`, `agent_handle_tool_result`, `agent_factory_reset` |
| **Agent Session Management** | 11 | `agent_get_session`, `agent_list_sessions`, `agent_delete_session` |
| **MCP Server Config** | 5 | `create_mcp_server_config`, `list_mcp_server_configs`, `list_mcp_server_presets` |
| **Messages** | 6 | `messages_get_page`, `messages_upsert`, `messages_search`, `messages_delete` |
| **Settings** | 5 | `set_setting`, `get_setting`, `list_settings`, `delete_setting` |
| **Migration** | 3 | `export_migration`, `import_migration`, `inspect_migration` |
| **Assistant CRUD** | 7 | `create_assistant`, `update_assistant`, `list_assistants`, `search_assistants` |
| **Playbook CRUD** | 6 | `create_playbook`, `update_playbook`, `list_playbooks`, `delete_playbook` |
| **Scheduled Tasks** | 10 | `create_scheduled_task`, `list_scheduled_tasks`, `update_scheduled_task`, `toggle_scheduled_task` |
| **Browser** | 9 | `create_browser_session`, `navigate_to_url`, `execute_script`, `close_browser_session` |
| **OAuth** | 4 | `has_oauth_token`, `get_oauth_token`, `revoke_oauth_token`, `start_oauth_flow` |
| **Knowledge** | 4 | `list_global_knowledge`, `get_global_knowledge_detail`, `delete_global_knowledge`, `get_global_knowledge_graph` |
| **File/Workspace** | 10 | `list_workspace_files`, `read_workspace_file_content`, `workspace_write_file`, `get_workspace_override` |
| **Download** | 6 | `download_media_file`, `download_text_file`, `download_binary_file`, `export_and_download_zip` |
| **Log** | 8 | `backup_current_log`, `list_log_files`, `log_trace`, `log_error_from_frontend` |
| **Skill Management** | 11 | `scan_skills_directory`, `get_aggregated_skills`, `import_user_skills`, `delete_user_skill` |
| **Dataset** | 1 | `export_dataset` |
| **Other** | 6 | `greet`, `restart_app`, `probe_mcp_server`, `open_external_url`, `media_assist_plugin_status` |
| **Bundle** | 3 | `export_org_bundle`, `import_org_bundle`, `inspect_org_bundle` |
| **URL** | 1 | (registered but minimal usage) |

### 1.2 Command Input Patterns

| Pattern | Description | Files |
|---------|-------------|-------|
| `Option<T>` with `unwrap_or` | Most optional params use `.unwrap_or(default)` — safe but inconsistent defaults | session_commands.rs, knowledge_commands.rs, dataset_commands.rs |
| Direct `String`/`Value` | Required params have no validation beyond serde deserialization | settings_commands.rs, mcp_server_config_commands.rs |
| `State<'_, T>` | Shared state injection via Tauri state management | agent_commands/*, all repo-backed commands |
| `window: tauri::Window` | Event emission for progress/callbacks | migration_commands/*, lifecycle/* |

---

## 2. TypeScript Client Layer

### 2.1 `safeInvoke<T>()` — Centralized Wrapper

**File**: `src/lib/backend/core.ts`

```typescript
export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
  options?: SafeInvokeOptions,
): Promise<T> {
  // ... logging ...
  const result = await invoke<T>(cmd, args ?? {});
  return result;
} catch (err) {
  logger.error('invoke failed', { cmd, err });
  throw err instanceof Error ? err : new Error(String(err));
}
```

**Analysis**:

| Aspect | Status | Notes |
|--------|--------|-------|
| Centralization | ✅ Good | All IPC goes through this single wrapper |
| Error logging | ✅ Good | Logs command name + args + error for every failure |
| Error propagation | ⚠️ Partial | Errors are re-thrown but callers rarely use `.catch()` |
| Type safety | ⚠️ Partial | `T` is generic but callers pass `unknown` or loose types |
| Performance tracking | ✅ Good | `recordStartupIpcCall()` measures latency |
| Args sanitization | ⚠️ Partial | `loggedArgs` option exists but is inconsistently used |

### 2.2 TypeScript Backend Module Inventory

| File | Commands Covered | Notes |
|------|-----------------|-------|
| `src/lib/backend/mcp-server-config.ts` | `create_mcp_server_config`, `update_mcp_server_config`, `delete_mcp_server_config`, `list_mcp_server_configs`, `list_mcp_server_presets` | Custom `deserializeMCPServer()` / `serializeMCPServer()` |
| `src/lib/backend/file-operations.ts` | File read/write operations | Uses `safeInvoke` for workspace file operations |
| `src/features/*/backend.ts` | Feature-specific commands | Various feature modules (agent, chat, settings, etc.) |
| `src/lib/backend/core.ts` | All commands (base) | Generic invoke wrapper |

---

## 3. Type Mapping & DTO Analysis

### 3.1 Rust DTO → TypeScript Interface Mapping

| Rust DTO | TypeScript Equivalent | Mapping Status |
|----------|----------------------|----------------|
| `SettingDto` | (inline in callers) | ⚠️ `value: Value` → `unknown` / `JSON` |
| `MCPServerDto` | `MCPServerEntity` | ⚠️ Custom deserialize with `as Partial<MCPServerEntity>` |
| `SessionMetadata` | `AgentSessionMetadata` | ✅ Good — fields aligned, `#[serde(rename_all = "camelCase")]` |
| `CreateAgentSessionRequest` | `CreateAgentSessionRequest` | ✅ Good — explicit interface |
| `RustMessage` | `RustMessage` | ✅ Good — documented conversion function |
| `ToolExecutionResult` | `ToolExecutionResult` | ✅ Good — explicit interface |
| `MigrationExportInfo` | (missing) | 🔴 No TypeScript type |
| `MigrationImportResult` | (missing) | 🔴 No TypeScript type |
| `AgentResponse<T>` | `AgentResponse<T>` | ✅ Good — generic wrapper |
| `KnowledgeChunkListItemDto` | (missing) | 🔴 No TypeScript type |
| `DeleteGlobalKnowledgeResponse` | (missing) | 🔴 No TypeScript type |

### 3.2 Field-Level Type Gaps

#### Gap 1: `SettingDto.value: serde_json::Value`

**Rust** (`src-tauri/src/commands/settings_commands.rs:9-12`):
```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingDto {
    pub key: String,
    pub value: Value, // JSON — arbitrary
    pub created_at: i64,
    pub updated_at: i64,
}
```

**Issue**: `serde_json::Value` serializes to arbitrary JSON. The TypeScript frontend receives `value: unknown` (via `Record<string, unknown>` in `set_setting` calls). No schema validation exists on the frontend side.

**Impact**: If the backend stores unexpected JSON (e.g., a nested object where a string was expected), the frontend will blindly pass it to React state without validation.

**Recommendation**: Add a Zod schema per setting key pattern, or define a discriminant union type for known setting shapes.

#### Gap 2: `MCPServerDto.config: serde_json::Value`

**Rust** (`src-tauri/src/commands/mcp_server_config_commands.rs:14`):
```rust
pub struct MCPServerDto {
    pub config: Value, // JSON config
    // ...
}
```

**TypeScript** (`src/lib/backend/mcp-server-config.ts:42-56`):
```typescript
function deserializeMCPServer(dto: MCPServerDto): MCPServerEntity {
  const config = dto.config as Partial<MCPServerEntity>;
  return {
    ...config,
    id: dto.id,
    name: dto.name,
    // ...
  } as MCPServerEntity;
}
```

**Issue**: The entire `config` JSON blob is `as`-cast to `Partial<MCPServerEntity>`. If the backend stores malformed config (e.g., missing `transport` field, wrong `transport.type` value), the cast silently produces a partially-invalid `MCPServerEntity`.

**Impact**: An MCP server with a corrupted config could cause silent failures during tool execution or OAuth flows.

**Recommendation**: Add a Zod schema for `MCPServerEntity` and validate during `deserializeMCPServer()`.

#### Gap 3: `RustMessage` ↔ `Message` Timestamp Conversion

**Rust** → `createdAt: i64` (Unix milliseconds)  
**TypeScript** `RustMessage` → `createdAt: number` (expects milliseconds)  
**TypeScript** `Message` → `createdAt: Date`

**Conversion** (`src/models/chat.ts:273-274`):
```typescript
createdAt: new Date(rustMsg.createdAt),
updatedAt: new Date(rustMsg.updatedAt),
```

**Analysis**: This is **correct** — `new Date(milliseconds)` is the right conversion. However, if the backend ever sends seconds instead of milliseconds, dates will be off by 1000×.

**Recommendation**: Add a runtime assertion: `if (rustMsg.createdAt > 1e12) { /* seconds → ms */ }`.

#### Gap 4: `ExecutionMode` String↔Enum Mapping

**Rust** (`src/execution_mode.rs`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionMode {
    Normal,
    Yolo,
    Unsafe,
}
```

**TypeScript** (`src/lib/generated/execution-mode.ts`):
```typescript
export const EXECUTION_MODE_VALUES = ['normal', 'yolo', 'unsafe'] as const;
export type ExecutionMode = (typeof EXECUTION_MODE_VALUES)[number];
```

**Analysis**: ✅ **Good** — the TypeScript enum is auto-generated from the Rust source via `scripts/sync-execution-mode.cjs`, ensuring compile-time parity. The `isExecutionMode()` and `normalizeExecutionMode()` guards add runtime safety.

---

## 4. Deserialization Safety Audit

### 4.1 🔴 Critical: Silent `.unwrap_or(Value::Null)` in DTO Conversion

**Location 1**: `src-tauri/src/commands/settings_commands.rs:21`
```rust
impl From<crate::entity::settings::Model> for SettingDto {
    fn from(model: crate::entity::settings::Model) -> Self {
        Self {
            key: model.key,
            value: serde_json::from_str(&model.value).unwrap_or(Value::Null),
            // ...
        }
    }
}
```

**Location 2**: `src-tauri/src/commands/mcp_server_config_commands.rs:25`
```rust
impl From<crate::entity::mcp_server::Model> for MCPServerDto {
    fn from(model: crate::entity::mcp_server::Model) -> Self {
        Self {
            // ...
            config: serde_json::from_str(&model.config).unwrap_or(Value::Null),
            // ...
        }
    }
}
```

**Issue**: If the database contains corrupt JSON in the `value` or `config` columns, the `from()` impl silently replaces it with `Value::Null` (i.e., `null`). No error is logged, no warning is surfaced.

**Impact**: 
- A user's MCP server config could silently become `null`, breaking all tool calls.
- A user's settings could silently disappear, causing unexpected defaults.
- Data loss is silent — the user has no indication their data was corrupted.

**Recommendation**: 
1. Log a warning: `warn!("Corrupt JSON in settings key '{}', defaulting to null", model.key);`
2. Consider returning an `Err(String)` from the command instead of silently discarding data.
3. Add a database integrity check command that scans for corrupt JSON rows.

### 4.2 🟡 High: `.unwrap()` on JSON Serialization in Migration Export

**Location**: `src-tauri/src/commands/migration_commands/export.rs:166, 174, 184, 194, 204, 214`

```rust
zip.write_all(serde_json::to_string_pretty(&manifest).unwrap().as_bytes())
zip.write_all(serde_json::to_string_pretty(&cleaned_settings).unwrap().as_bytes())
zip.write_all(serde_json::to_string_pretty(&cleaned_assistants).unwrap().as_bytes())
// ... 4 more .unwrap() calls
```

**Issue**: `serde_json::to_string_pretty()` can theoretically fail (e.g., if a Value contains a non-serializable type, though this is extremely rare). The `.unwrap()` will panic the entire export operation.

**Impact**: A single corrupt record in any table (settings, assistants, MCP servers, playbooks, scheduled tasks) will crash the entire migration export, potentially causing users to lose the opportunity to back up other valid data.

**Recommendation**: Replace `.unwrap()` with `.map_err(|e| format!("Failed to serialize {}: {}", section, e))?`. The migration export should fail with a descriptive error rather than panic.

### 4.3 🟡 High: Password `unwrap_or("")` Fallback in Migration Import

**Location 1**: `src-tauri/src/commands/migration_commands/import.rs:53`
```rust
format_version = v
    .get("format_version")
    .and_then(|x| x.as_u64())
    .unwrap_or(1) as u32;
```

**Location 2**: `src-tauri/src/commands/migration_commands/export.rs:251`
```rust
let pwd = password.as_deref().unwrap_or("");
let encrypted_bytes = encrypt_data(&zip_bytes, pwd)?;
```

**Location 3**: `src-tauri/src/commands/migration_commands/org_bundle.rs:298`
```rust
let pwd = password.as_deref().unwrap_or("");
```

**Issue**: When `password` is `None`, the code falls back to an empty string `""`. This means:
- For **export**: encrypted data is encrypted with an empty password (trivially breakable).
- For **import**: if a user accidentally passes `None` for an encrypted file, the decryption will fail — but the error message may be confusing.

**Impact**: A user who forgets to provide a password during export gets "encrypted" data with zero protection. The data is as secure as a plain-text file.

**Recommendation**: For export, reject `password.is_none()` when `include_sensitive_data` is `true` (this check already exists at line 52 of export.rs). For import, ensure the error message clearly states "password required for encrypted backup".

### 4.4 🟡 High: `.unwrap()` in `mcp_commands.rs` Host Extraction

**Location**: `src-tauri/src/commands/mcp_commands.rs:138`
```rust
let host = parsed_url.host_str().unwrap_or("");
```

**Issue**: If `host_str()` returns `None` (e.g., for an IPv6 address without brackets, or a bare IP without a hostname), the host is set to `""` and the subsequent check `host != "localhost" && host != "127.0.0.1"` will correctly reject it. However, the empty string fallback is misleading — the error message says "host '' is not allowed" rather than explaining the real issue.

**Impact**: Minor — security is correct, but the error message is confusing.

**Recommendation**: Use `parsed_url.host_str().unwrap_or("<invalid>")` and improve the error message.

---

## 5. Event System Analysis

### 5.1 Backend Event Emission Inventory

| Event Name | Source File | Emission Target | Payload |
|------------|-------------|-----------------|---------|
| `migration:progress` | `migration_commands/export.rs`, `import.rs` | `window` | `u32` progress percentage |
| `org-bundle:progress` | `migration_commands/org_bundle.rs` | `window` | `u32` progress percentage |
| `agent:event` | `services/` (various) | `app_handle` | `GlobalEventPayload` (resourceUpdated) |
| `oauth-callback` | `lifecycle/app_setup/mod.rs:352` | `app_handle` | `&str` URL string |
| `llm:completion-request` | `agent/llm/completion/request/orchestration.rs:246` | `app_handle` | Completion request struct |
| `llm:compact-state` | `agent/tauri_events.rs:52` | `app_handle` | Compact state event |
| `llm:compact-request` | `agent/tauri_events.rs:97` | `app_handle` | Compact request event |
| `llm:completion-cancel` | `agent/tauri_events.rs:106` | `app_handle` | Cancel request event |
| `frontend-ready` | `lifecycle/app_setup/mod.rs:358` | `app_handle` | (empty) |

### 5.2 Frontend Event Subscription Analysis

**File**: `src/context/GlobalEventContext.tsx`

The frontend subscribes to events via the `listen()` API from `@tauri-apps/api/event`:

```typescript
const cleanup = await listen<GlobalEventPayload>(
  'agent:event',  // ← Only this event is listened to
  (event) => { ... }
);
```

### 5.3 🔴 Critical: Missing Event Listeners

| Backend Event | Frontend Listener | Impact |
|---------------|-------------------|--------|
| `migration:progress` | ❌ None | Migration progress bar never updates — user sees no feedback |
| `org-bundle:progress` | ❌ None | Same as above for org bundle import/export |
| `oauth-callback` | ❌ None | OAuth flow completion may not propagate to UI |
| `llm:completion-request` | ❌ None | Frontend cannot track LLM request lifecycle |
| `llm:compact-state` | ❌ None | Frontend cannot show compaction progress |
| `llm:compact-request` | ❌ None | Frontend cannot show compaction in progress |
| `llm:completion-cancel` | ❌ None | Frontend cannot show cancellation status |
| `frontend-ready` | ❌ None (backend-only) | Backend logs readiness, but frontend doesn't react |

**Impact**: The migration progress bar is the most visible gap — users trigger a migration export/import and see no progress feedback until completion (or failure). This is a significant UX issue.

**Recommendation**: 
1. Create a `src/lib/backend/events.ts` module that registers listeners for all backend events.
2. Wire `migration:progress` to a progress state in the migration UI.
3. Wire `llm:*` events to the agent workflow UI for real-time status updates.

### 5.4 Event Error Handling

All backend `.emit()` calls use `.ok()` to suppress errors:

```rust
window.emit("migration:progress", 5).ok();  // ← Error silently ignored
app_handle.emit("oauth-callback", url_str).ok();  // ← Error silently ignored
```

**Issue**: If the event channel is full or the frontend has disconnected, the error is silently swallowed. The user gets no indication that progress updates are not arriving.

**Recommendation**: At minimum, log errors: `if let Err(e) = window.emit(...) { warn!("Event emit failed: {}", e); }`.

---

## 6. Security Analysis

### 6.1 Security-Sensitive Commands

| Command | Risk Level | Security Mechanism |
|---------|-----------|-------------------|
| `start_oauth_flow` | 🔴 High | Loopback-only redirect URI validation, state parameter, keychain storage |
| `export_migration` | 🔴 High | Password encryption for sensitive data, format version check |
| `import_migration` | 🔴 High | DB backup before import, transactional writes, format compatibility check |
| `agent_factory_reset` | 🔴 High | Irreversible — destroys all session data |
| `set_workspace_override` | 🟡 Medium | Allows arbitrary filesystem path — could access sensitive directories |
| `submit_interactive_shell_input` | 🟡 Medium | Shell execution — potential for command injection if params are unsanitized |
| `workspace_write_file` | 🟡 Medium | File write to workspace — path traversal risk if workspace path is not validated |
| `delete_mcp_server_config` | 🟢 Low | Requires ID — no auth boundary (single-user desktop app) |

### 6.2 OAuth Flow Security

**Location**: `src-tauri/src/commands/mcp_commands.rs:116-170`

| Check | Status |
|-------|--------|
| Redirect URI must be `http://` | ✅ |
| Host must be `localhost` or `127.0.0.1` | ✅ (with `unwrap_or("")` caveat — see §4.4) |
| Port must be specified | ✅ |
| State parameter for CSRF protection | ✅ |
| Token stored in OS keychain | ✅ |
| Loopback server for auth code exchange | ✅ |

**Assessment**: The OAuth flow is well-implemented and follows best practices. The only minor issue is the `unwrap_or("")` on `host_str()` which could produce a confusing error message but does not weaken security.

### 6.3 Migration Security

| Check | Status |
|-------|--------|
| Sensitive data masking during export | ✅ (`clean_settings`, `clean_mcp_servers`, `clean_assistants`) |
| Password-protected encryption | ✅ (password required for sensitive export) |
| DB backup before import | ✅ (`BackupManager::create_backup`) |
| Transactional import | ✅ (`db.begin()`) |
| Foreign key management | ✅ (`PRAGMA foreign_keys = OFF` during import, re-enabled after) |
| Format version compatibility check | ✅ |

### 6.4 No Input Validation Layer

**Critical Gap**: None of the ~87 Tauri commands perform input validation beyond what serde derive macros provide. There is no:
- Length validation on strings
- Regex validation on IDs
- Range validation on numeric parameters
- Path traversal prevention on file paths
- URL scheme validation (except OAuth)

**Recommendation**: Add a validation layer using a crate like `validator` or manual guard functions for sensitive commands (file operations, workspace override, shell input).

---

## 7. Error Handling Analysis

### 7.1 Backend Error Returns

All commands return `Result<T, String>`:

```rust
#[command]
pub async fn set_setting(key: String, value: Value) -> Result<SettingDto, String> {
    // ...
    .map_err(|e| format!("Failed to set setting: {}", e))?
}
```

**Issue**: Error messages are unstructured strings. The frontend `safeInvoke` throws them as `Error` objects with no type discrimination.

**Impact**: The frontend cannot distinguish between "not found", "permission denied", "database error", etc. — all come through as generic `Error` objects.

**Recommendation**: Define a Rust error enum (e.g., `AppError`) that serializes to a structured JSON object with `code`, `message`, and optional `details`. Update `safeInvoke` to parse and re-throw with typed error classes.

### 7.2 Frontend Error Handling

```typescript
// Most callers do NOT use .catch():
const result = await safeInvoke<SomeType>('some_command', { param });
// If this throws, it propagates to the nearest try/catch (if any)
```

**Issue**: Without `.catch()` handlers, errors bubble up to the React error boundary or the top-level `unhandledrejection` handler. This means:
- Command failures in non-UI contexts (e.g., background sync) may go unnoticed.
- Error recovery (retry, fallback) is not implemented.

**Recommendation**: Implement a global error handler that routes IPC errors to appropriate UI notifications or retry logic.

### 7.3 Event `.ok()` Error Swallowing

As noted in §5.4, all `.emit()` calls use `.ok()`, which discards errors. This is acceptable for fire-and-forget events (like progress updates) but problematic for critical events (like OAuth callbacks).

---

## 8. Recommendations

### 8.1 Priority 1 (Critical — Fix Immediately)

| # | Recommendation | Effort | Files |
|---|---------------|--------|-------|
| 1 | Replace `.unwrap_or(Value::Null)` with logged fallback in `settings_commands.rs` and `mcp_server_config_commands.rs` | Low | 2 files |
| 2 | Add frontend event listener for `migration:progress` | Medium | New file + migration UI |
| 3 | Add Zod validation in `deserializeMCPServer()` | Medium | `mcp-server-config.ts` |

### 8.2 Priority 2 (High — Fix Before Next Release)

| # | Recommendation | Effort | Files |
|---|---------------|--------|-------|
| 4 | Replace `.unwrap()` with `.map_err()` in migration export serialization | Low | `export.rs` |
| 5 | Create TypeScript types for `MigrationExportInfo`, `MigrationImportResult` | Low | New types file |
| 6 | Add event listeners for `llm:*` events | Medium | New event handler |
| 7 | Add input validation layer for sensitive commands | High | Multiple files |
| 8 | Define structured error enum for Tauri commands | Medium | New error module |

### 8.3 Priority 3 (Medium — Improve Over Time)

| # | Recommendation | Effort | Files |
|---|---------------|--------|-------|
| 9 | Add `.catch()` handlers to critical `safeInvoke` calls | Low | Various |
| 10 | Add timestamp sanity check in `rustMessageToMessage()` | Low | `chat.ts` |
| 11 | Log event emission errors instead of `.ok()` | Low | Multiple files |
| 12 | Add database integrity check command | Medium | New command |

---

## 9. Summary Statistics

| Metric | Value |
|--------|-------|
| Total Tauri commands | ~87 |
| TypeScript backend modules | ~5 (core + feature-specific) |
| Backend events emitted | ~9 unique names |
| Frontend event listeners | 1 (`agent:event` only) |
| `.unwrap()` calls in commands | 6 (migration export serialization) |
| `.unwrap_or(Value::Null)` calls | 2 (settings, MCP config DTO conversion) |
| `.unwrap_or("")` on passwords | 3 (export, import, org bundle) |
| Commands with no frontend type | ~8 |
| Security-sensitive commands | 4 (OAuth, migration, factory reset, shell) |

---

## 10. Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        TypeScript Frontend                      │
│                                                                 │
│  ┌──────────────┐    ┌──────────────────┐    ┌───────────────┐ │
│  │ safeInvoke() │───▶│ feature/ backend │───▶│ GlobalEvent   │ │
│  │ core.ts      │    │ modules          │    │ Context.tsx   │ │
│  └──────────────┘    └──────────────────┘    └───────┬───────┘ │
│       │                                               │         │
│       │  invoke<T>()         listen('agent:event')     │         │
│       ▼                                               ▼         │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Tauri invoke_handler                        │  │
│  └──────┬───────────────────────────────────┬──────────────┘  │
└─────────┼───────────────────────────────────┼─────────────────┘
          │                                   │
          │  Rust Result<T, String>           │  app_handle.emit()
          ▼                                   ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Rust Backend (Tauri)                        │
│                                                                 │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ agent_commands │  │ migration_   │  │ lifecycle/         │  │
│  │ session/cmds   │  │ commands/    │  │ app_setup/mod.rs   │  │
│  │ workflow/cmds  │  │ export/import│  │ OAuth deep-link    │  │
│  └────────────────┘  └──────────────┘  └────────────────────┘  │
│                                                                 │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ mcp_commands   │  │ settings/    │  │ knowledge/         │  │
│  │ OAuth + probe  │  │ commands     │  │ commands           │  │
│  └────────────────┘  └──────────────┘  └────────────────────┘  │
│                                                                 │
│  ┌────────────────┐  ┌──────────────┐  ┌────────────────────┐  │
│  │ repositories/  │  │ agent/       │  │ mcp/               │  │
│  │ SeaORM models  │  │ LLM + events │  │ keychain + OAuth   │  │
│  └────────────────┘  └──────────────┘  └────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```
