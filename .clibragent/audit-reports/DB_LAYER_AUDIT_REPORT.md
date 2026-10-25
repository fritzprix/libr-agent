# SQLite / Data-Access Layer Audit Report

**Date**: 2026-10-18  
**Auditor**: Coding Expert (Sub-Agent, Session `e5e5bca121`)  
**Scope**: Backend repositories, Tauri command layer, frontend wrappers, schema management  
**Verdict**: Solid architecture with manageable improvement candidates. No critical blockers.

---

## 1. Architecture Overview

### 1.1 State Management (`src-tauri/src/state.rs`)

All application state is managed through **global `OnceLock` singletons**:

| Singleton                    | Type                             | Purpose                          |
| ---------------------------- | -------------------------------- | -------------------------------- |
| `DATABASE_CONNECTION`        | `DatabaseConnection`             | SQLite connection (SeaORM)       |
| `MESSAGE_REPOSITORY`         | `SqliteMessageRepository`        | Message CRUD + search            |
| `PENDING_QUEUE_REPOSITORY`   | `SqlitePendingQueueRepository`   | Agent message queue              |
| `SESSION_REPOSITORY`         | `SqliteSessionRepository`        | Session lifecycle                |
| `SETTINGS_REPOSITORY`        | `SqliteSettingsRepository`       | App settings                     |
| `MCP_SERVER_REPOSITORY`      | `SqliteMCPServerRepository`      | MCP server config                |
| `ASSISTANT_REPOSITORY`       | `SqliteAssistantRepository`      | Assistant definitions            |
| `PLAYBOOK_REPOSITORY`        | `SqlitePlaybookRepository`       | Playbook definitions             |
| `KNOWLEDGE_REPOSITORY`       | `SqliteKnowledgeRepository`      | Legacy knowledge                 |
| `KNOWLEDGE_V2_REPOSITORY`    | `SqliteKnowledgeV2Repository`    | Hybrid knowledge (BM25 + vector) |
| `PLANNING_REPOSITORY`        | `SqlitePlanningRepository`       | Goal/todo/scratchpad             |
| `SCHEDULED_TASK_REPOSITORY`  | `SqliteScheduledTaskRepository`  | Cron tasks                       |
| `COMPACT_CONTEXT_REPOSITORY` | `SqliteCompactContextRepository` | Context compaction               |
| `ATTACHMENTS_REPOSITORY`     | `SqliteAttachmentsRepository`    | File attachments                 |

Plus runtime-only singletons: `MCP_SERVICE_PROXY_MANAGER`, `SQLITE_DB_URL`, `APP_HANDLE`, `SESSION_BUS`, `CONCURRENCY_GATE`, `ACTIVE_SESSIONS` (HashMap), `CHANNEL_DISPATCH_AGENT`, `SKILLS_CATALOG_REVISION`, `MANAGED_SKILLS_SYNC_STATE`, `STARTUP_TIMER`.

### 1.2 Repository Trait Pattern

Every repository follows **trait + SQLite implementation**:

```
mod.rs          → exports trait + impl types
sqlite.rs       → Sqlite*Repository impl (where complex)
types.rs        → domain types, cursors, pages
persist.rs      → DB row ↔ domain mapping
index_meta.rs   → search index metadata
search.rs       → hybrid search (knowledge v2)
graph.rs        → CTE graph traversal (knowledge v2)
cleanup.rs      → garbage collection (knowledge v2)
contracts.rs    → shared contracts (knowledge v2)
detail.rs       → detail retrieval (knowledge v2)
```

The trait provides a clean abstraction for:

- Testing (InMemorySessionRepository exists)
- Potential future storage backends
- Consistent method signatures

### 1.3 Error Handling (`src-tauri/src/repositories/error.rs`)

`DbError` enum with 7 variants:

- `SeaOrmQueryFailed` — SeaORM query errors
- `NotFound` — Record not found
- `InvalidInput` — Bad input
- `TransactionFailed` — Transaction commit/rollback failure
- `SerializationError` — JSON serde failure
- `DuplicateResource` — Already exists
- `ResourceNotFound` — General not found

**Key method**: `is_sqlite_busy()` — detects SQLite BUSY/LOCKED errors by checking error codes (`5`, `6`, `SQLITE_BUSY`, `SQLITE_LOCKED`) and message strings (`"database is locked"`, `"database table is locked"`). This enables retry logic at the call site.

### 1.4 Schema Validation (`src-tauri/src/db_schema_validator.rs`)

Runtime schema validator that:

- Checks core tables exist (`sessions`, `messages`, `assistants`)
- Validates specific columns on `sessions`, `planning_goals`, `planning_todos`, `planning_scratchpad`
- SQL injection protection: validates table names against `[a-zA-Z0-9_]+` regex
- Includes unit tests with in-memory SQLite DB
- **Gap**: Only validates a subset of tables; many entities (knowledge, pending_queue, settings, mcp_server, playbook, scheduled_task, attachments, compact_context, assistant) are NOT validated at runtime.

---

## 2. Strengths

### 2.1 Consistent Trait Abstraction

All 13 repositories implement a trait. This enables:

- `InMemorySessionRepository` for test isolation
- Easy mocking in tests
- Future-proofing against storage backend changes

### 2.2 No N+1 Queries in Hot Paths

Session recovery (`recovery.rs`) iterates `get_all_sessions()` but each session only loads a **recent message slice** (`MAX_CACHED_MESSAGES` constant), not the full message history. This is appropriate for a local desktop app.

### 2.3 Modular Message Repository

The message repository is split into focused modules:

- `types.rs` — public cursors/pages, `MessageRepository` trait
- `persist.rs` — DB row ↔ domain mapping, error-column UI envelope
- `index_meta.rs` — search index metadata per session
- `sqlite.rs` — query orchestration

### 2.4 Knowledge v2 Hybrid Search

Knowledge v2 repository implements **BM25 + vector L2 distance** hybrid search:

- BM25 for text relevance ranking
- Vector distance for semantic similarity
- CTE-based graph traversal for entity/relationship queries
- Atomic deletion and cleanup

### 2.5 Pending Queue Transactional Integrity

The pending queue uses atomic operations:

- Monotonic `queue_seq` allocation
- `remove_returning` to preserve seq during claim/cancel
- `remove_index_and_message` for atomic cascade
- `commit_merged_claim` for atomic merge
- `delete_orphans_for_session` for cleanup

### 2.6 SQLite Busy Detection

The `DbError::is_sqlite_busy()` method enables callers to implement retry logic for transient SQLite lock contention.

### 2.7 Planning Repository Serialized Writes

Uses `run_serialized_write` with a global `Mutex<()>` to prevent SQLite "database is locked" errors during concurrent planning writes from multiple agent sessions.

### 2.8 Frontend Type Safety

`src/lib/backend/messages.ts` and other backend wrappers:

- Use `safeInvoke()` for centralized error handling
- Proper TypeScript type definitions
- Deserialization from Rust format with fallbacks
- Input validation before sending to backend

---

## 3. Improvement Candidates

### 3.1 P1 — Expand Schema Validation Coverage

**Risk**: Medium-High  
**Files**: `src-tauri/src/db_schema_validator.rs`

The validator only checks 4 tables. The application has **16+ tables** (sessions, messages, assistants, mcp_servers, settings, knowledge_chunks, knowledge_chunks_entities, knowledge_chunks_relationships, knowledge_v2_graph, knowledge_v2_entities, knowledge_v2_relationships, planning_goals, planning_todos, planning_scratchpad, pending_queue, playbooks, scheduled_tasks, attachments, compact_context, assistant).

**Recommendation**: Extend `validate_schema()` to check all tables and columns. This should run at app startup as a guard against migration failures or manual DB corruption.

### 3.2 P1 — Replace `OnceLock` with `LazyLock` in Repositories

**Risk**: Medium  
**Files**: `src-tauri/src/repositories/planning_repository.rs` (uses `OnceLock`)

`OnceLock` is deprecated in favor of `LazyLock` in stable Rust. The planning repository uses `OnceLock` for its write serialization mutex.

**Recommendation**: Replace `use std::sync::OnceLock` with `use std::sync::LazyLock` and update initialization.

### 3.3 P2 — Transaction Granularity Inconsistency

**Risk**: Medium  
**Files**: Multiple repository implementations

Transaction handling varies across repositories:

- **Planning**: Uses `run_serialized_write` with global `Mutex<()>`
- **Pending Queue**: Explicit `begin()` / `commit()` per operation
- **Knowledge v2**: Varies by method
- **Session**: Per-operation transactions

**Recommendation**: Define a clear transaction policy:

- Short-lived operations: per-operation transactions
- Multi-entity operations: explicit transactions with proper rollback
- Consider adding a `TransactionGuard` wrapper type

### 3.4 P2 — Missing Indexes

**Risk**: Medium  
**Files**: Entity definitions, migrations

Several query patterns may lack optimal indexing:

- `sessions` table: search by `assistant_id` involves a subquery against `assistants` table (see `session_search_condition` in `session_repository.rs`)
- `knowledge_v2` graph queries: CTE traversal may benefit from entity/relationship indexes
- `pending_queue`: `queue_seq` monotonic allocation uses `MAX()` query

**Recommendation**: Profile queries under load and add indexes where needed. The subquery in session search (`assistant_id IN (SELECT id FROM assistants WHERE ...)`) is particularly expensive for large assistant collections.

### 3.5 P2 — Session List In-Memory Pagination

**Risk**: Low-Medium  
**Files**: `src/lib/backend/session-crud.ts`

The frontend `getSessionsPage()` loads **all sessions into memory** and slices client-side. This works for local desktop use but becomes problematic if session count grows significantly (thousands of sessions).

**Recommendation**: If session count exceeds ~500, implement cursor-based pagination on the Rust side and use it in the frontend.

### 3.6 P2 — Error Handling Granularity

**Risk**: Low-Medium  
**Files**: `src-tauri/src/repositories/error.rs`, command handlers

`DbError` is good but some command handlers convert all errors to generic `String` via `Result<T, String>`. This loses structured error information.

**Recommendation**: Consider using `anyhow::Result` or a custom `AppError` type that preserves error context through the command layer, rather than converting to `String` at every boundary.

---

## 4. Command Layer Analysis

### 4.1 Command Files (`src-tauri/src/commands/`)

40 command files covering:

- **Agent operations**: `agent_commands/` (session, compaction, workflow, channel, contracts, ui_actions) — 6 files
- **Core CRUD**: `session_commands.rs`, `messages_commands.rs`, `knowledge_commands.rs`, `assistant_crud_commands.rs`, `playbook_commands.rs`, `scheduled_task_commands.rs`
- **Configuration**: `settings_commands.rs`, `mcp_server_config_commands.rs`
- **Data management**: `migration_commands/` (8 files: sanitize, export, crypto, import, inspect, reverify, org_bundle, zip_fs), `dataset_commands.rs`
- **File/media**: `workspace_commands.rs`, `file_commands.rs`, `attachments_commands.rs`, `media_assist_commands.rs`, `markdown_pdf.rs`, `url_commands.rs`, `download_commands.rs`
- **Tools**: `browser_commands.rs`, `mcp_commands.rs`, `skill_commands.rs`, `skill_management.rs`
- **Output**: `session_export_commands.rs`, `log_commands.rs`

### 4.2 Frontend Wrappers (`src/lib/backend/`)

49 TypeScript files. Key database-related wrappers:

- `messages.ts` (8.38 KB) — message CRUD, BM25 search, cursor pagination
- `session-crud.ts` (5.07 KB) — session lifecycle, in-memory pagination
- `settings.ts` (2.56 KB) — settings CRUD
- `knowledge.ts` (2.80 KB) — knowledge operations
- `mcp-server.ts` (3.47 KB) — MCP server config
- `playbooks.ts` (7.42 KB) — playbook CRUD
- `scheduled-tasks.ts` (3.67 KB) — cron task CRUD
- `agent-commands.ts` (6.99 KB) — agent session operations
- `workspace.ts` (6.73 KB) — file operations
- `assistants.ts` (4.63 KB) — assistant CRUD

All use `safeInvoke()` from `core.ts` for centralized Tauri command invocation with error handling.

### 4.3 Message Deserialization

`messages.ts` includes a `deserializeMessage()` function that handles:

- Field mapping (camelCase ↔ snake_case via Rust serde)
- Tool call `type` field fallback (`'function'`)
- Thread ID fallback to session ID
- Timestamp conversion to `Date` objects
- Attachment and metadata passthrough
- Source validation with `isMessageSource()` type guard

---

## 5. Risk Ranking

| Priority | Issue                                  | Impact                                             | Effort  |
| -------- | -------------------------------------- | -------------------------------------------------- | ------- |
| **P0**   | None identified                        | —                                                  | —       |
| **P1**   | Expand schema validation to all tables | Data corruption risk from migration failures       | Low     |
| **P1**   | Replace `OnceLock` with `LazyLock`     | Compiler warnings, future incompatibility          | Trivial |
| **P2**   | Transaction granularity inconsistency  | Potential data loss on crash during multi-step ops | Medium  |
| **P2**   | Missing indexes on search queries      | Performance degradation with large datasets        | Medium  |
| **P2**   | Session list in-memory pagination      | OOM risk with thousands of sessions                | Low     |
| **P2**   | Error handling string conversion       | Debugging difficulty, lost context                 | Low     |

---

## 6. Implementation Priority List

1. **[P1]** Add all-table schema validation in `db_schema_validator.rs` — run at app startup
2. **[P1]** Replace `OnceLock` → `LazyLock` in planning repository
3. **[P2]** Add indexes on `sessions.assistant_id`, `knowledge_v2` graph edges, `pending_queue.session_id+queue_seq`
4. **[P2]** Profile session search query; consider denormalizing `assistant_name` into sessions table
5. **[P2]** Implement cursor-based pagination for session list (Rust side + frontend)
6. **[P2]** Standardize transaction policy across all repositories
7. **[P2]** Consider `anyhow::Result` or typed error type for command handlers
8. **[Low]** Add `DbError` variants for domain-specific errors (duplicate, validation, etc.)
9. **[Low]** Add integration tests for repository layer (using `InMemorySessionRepository` pattern)
10. **[Low]** Document transaction boundaries in repository trait docs

---

## 7. Summary

The SQLite/data-access layer is **well-architected** for a local desktop application:

- ✅ **Trait-based abstraction** enables testability and future flexibility
- ✅ **No N+1 in hot paths** — appropriate for the scale of a local app
- ✅ **Hybrid search** (BM25 + vector) in knowledge v2 is a significant feature
- ✅ **Transactional integrity** in the pending queue is carefully designed
- ✅ **SQLite busy detection** enables retry logic
- ✅ **Frontend wrappers** are typed and use centralized error handling

**Primary improvement areas** are incremental: expand schema validation, replace deprecated `OnceLock`, add indexes for known query patterns, and standardize transaction policies. None of these are blockers — the current architecture is sound for the intended use case.
