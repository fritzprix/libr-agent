# Knowledge Tool: Assistant-Scoped → Shared Storage Migration

**Status**: Draft  
**Created**: 2025-01-XX  
**Target Version**: TBD  
**Component**: `src-tauri/src/mcp/builtin/knowledge/`  
**Estimated Effort**: Medium (2-3 days)

---

## 1. Problem Statement

### 1.1 Current Architecture

Knowledge is currently **scoped per assistant**. Each assistant gets its own isolated knowledge store via `assistant_id`:

```
Assistant A ─┬─ Chunk #1 (assistant_id = "aid-a")
             ├─ Chunk #2 (assistant_id = "aid-a")
             └─ Entity Graph (scoped to "aid-a")

Assistant B ─┬─ Chunk #3 (assistant_id = "aid-b")
             └─ Entity Graph (scoped to "aid-b")
```

`assistant_id` is injected at every layer:

| Layer               | File                          | Usage                                                                                |
| ------------------- | ----------------------------- | ------------------------------------------------------------------------------------ |
| **Factory**         | `service_proxy/factory.rs`    | Extracts from session → passes to `KnowledgeServer::new(assistant_id, db)`           |
| **Server struct**   | `knowledge/mod.rs`            | `assistant_id: String` field on `KnowledgeServer`                                    |
| **Tool handlers**   | `operations.rs`, `queries.rs` | Passed as `assistant_id: &str` to every function                                     |
| **Repository**      | `repository.rs`               | Every query uses `.filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))` |
| **Service Context** | `knowledge/mod.rs` L96-109    | Filters chunk count by `assistant_id`, outputs `Assistant ID: {id}` in prompt        |

### 1.2 What's Wrong

When an agent session starts, the system prompt should contain a `## Knowledge Base` section summarizing available knowledge. **It does not appear.** The section is missing entirely from the prompt dump.

Root cause: the factory extracts `assistant_id` from the session. If the session has no assistant configuration, `extract_assistant_id_from_session()` returns `None`, and the Knowledge server fails to initialize — silently skipped.

Even if it works, the design is flawed:

| Issue                                              | Impact                                                                                                   |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `assistant_id` output in context serves no purpose | The AI already knows its own identity; repeating it adds tokens without value                            |
| Per-assistant isolation blocks knowledge reuse     | Assistant A cannot discover what Assistant B stored; no shared memory                                    |
| No content hints                                   | The context only shows a count (`Stored Chunks: 42`). The AI has no idea what's actually in the database |

### 1.3 What the System Prompt Currently Shows

```
## Available Tools & Current State

## Planning
### Stable Context
- No goal set

### Live State
- Tasks: None

## Playbooks
Playbooks: None

## Attachments
### Live State
- No files available

## Workspace
### Live State
- Workspace Root: ...
```

**`## Knowledge Base` is absent.** No count, no tags, no hint of what's stored.

---

## 2. Design Goals

### 2.1 Primary Objective

**Convert Knowledge from assistant-scoped to shared storage.** All assistants share a single knowledge base. The `assistant_id` filter is removed from all queries.

### 2.2 Sub-Objectives

| #    | Goal                                                                                            | Priority |
| ---- | ----------------------------------------------------------------------------------------------- | -------- |
| SO-1 | Remove `assistant_id` from `KnowledgeServer` struct and all initialization paths                | P0       |
| SO-2 | Replace `assistant_id`-filtered queries with global (unfiltered) queries                        | P0       |
| SO-3 | Replace `Assistant ID: {id}` + `Stored Chunks: {count}` with actionable hints (tags, sources)   | P0       |
| SO-4 | Preserve `record_knowledge` / `prune_knowledge` semantics — existing data must remain queryable | P1       |
| SO-5 | Ensure `search_knowledge` and `explore_context` work across all shared data                     | P0       |

---

## 3. Current Code Structure

### 3.1 Layer-by-Layer Flow

```
┌──────────────────────────────────────────────────────────────────┐
│ 1. Factory (service_proxy/factory.rs)                           │
│    - Fetches session → extracts assistant_id                     │
│    - Calls KnowledgeServer::new(assistant_id, db)                │
│    - If extract_assistant_id_from_session() returns None → FAIL  │
└────────────────────────┬─────────────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────────────┐
│ 2. KnowledgeServer (knowledge/mod.rs)                            │
│    - Stores assistant_id: String                                 │
│    - get_service_context() queries with assistant_id filter      │
│    - call_tool() passes assistant_id to operations/queries       │
└────────────────────────┬─────────────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────────────┐
│ 3. Tool Handlers (operations.rs, queries.rs)                     │
│    - record_knowledge(assistant_id, ...)                         │
│    - search_knowledge(assistant_id, ...)                         │
│    - explore_context(assistant_id, ...)                          │
│    - prune_knowledge(assistant_id, ...)                          │
└────────────────────────┬─────────────────────────────────────────┘
                         │
┌────────────────────────▼─────────────────────────────────────────┐
│ 4. Repository (repository.rs)                                    │
│    - Every query: .filter(Column::AssistantId.eq(assistant_id))  │
│    - list_chunks() accepts Option<&str> (partial support)        │
│    - search_hybrid() accepts &str (hard filter)                  │
│    - get_graph_context() accepts &str (hard filter)              │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 Entity Schema

```rust
// src-tauri/src/entity/knowledge_chunk_v2.rs
pub struct Model {
    pub id: i32,
    pub assistant_id: String,   // ← Currently required for scoping
    pub content: String,
    pub tags: Option<String>,   // JSON array: ["tech", "project_alpha"]
    pub source: Option<String>, // e.g. "conversation", "https://..."
    pub created_at: i64,
}
```

### 3.3 Current `get_service_context()` Output

```rust
// knowledge/mod.rs L84-99
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let assistant_id = &self.assistant_id;
    let chunk_count = knowledge_chunk_v2::Entity::find()
        .filter(knowledge_chunk_v2::Column::AssistantId.eq(assistant_id))
        .count(self.db.as_ref())
        .await
        .ok();

    ServiceContext::new(format!(
        "# Knowledge Base\n\nAssistant ID: {}\nStored Chunks: {}",
        assistant_id,
        chunk_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    ))
    .with_volatility(ContextVolatility::Medium)
}
```

**Output example:**

```
# Knowledge Base

Assistant ID: aid-12345
Stored Chunks: 42
```

**Problems:**

- `Assistant ID` tells the AI nothing it doesn't already know
- `Stored Chunks: 42` is a number — no information about content
- If the query fails, it shows `"unknown"` — no fallback hint

---

## 4. Proposed Changes

### 4.1 KnowledgeServer — Remove `assistant_id`

```rust
// Before
pub struct KnowledgeServer {
    assistant_id: String,
    db: Arc<DatabaseConnection>,
}

impl KnowledgeServer {
    pub async fn new(assistant_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self { assistant_id, db })
    }
}

// After
pub struct KnowledgeServer {
    db: Arc<DatabaseConnection>,
}

impl KnowledgeServer {
    pub async fn new(db: Arc<DatabaseConnection>) -> Result<Self, String> {
        Ok(Self { db })
    }
}
```

### 4.2 Factory — Remove Session Lookup

```rust
// Before (factory.rs)
BuiltinServiceId::Knowledge => {
    let session = crate::get_session_repository()
        .get_session(&_session_id)
        .await
        .map_err(|e| format!("Database error fetching session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", _session_id))?;
    let assistant_id = crate::agent::extract_assistant_id_from_session(&session)
        .ok_or_else(|| "Session has no assistant configuration".to_string())?;
    Ok(Some(Box::new(
        crate::mcp::builtin::knowledge::KnowledgeServer::new(assistant_id, _db).await?,
    )))
}

// After
BuiltinServiceId::Knowledge => {
    Ok(Some(Box::new(
        crate::mcp::builtin::knowledge::KnowledgeServer::new(_db).await?,
    )))
}
```

**Impact:** Removes 4 lines of session lookup code. No more failure from missing assistant config.

### 4.3 Tool Handlers — Remove `assistant_id` Parameter

### 4.3 Tool Handlers — Dynamic Caller Identity

**CRITICAL FIX:** Do NOT remove `assistant_id` from tool handlers. Instead, resolve it dynamically from `session_id` at call time.

The `call_tool()` method already receives `_session_id: Option<String>`. We use this to look up the caller's `assistant_id` on-demand, rather than storing it in the server struct.

```rust
// Before
async fn call_tool(&self, tool_name: &str, args: Value, _session_id: Option<String>)
    -> Result<MCPResult, String>
{
    let assistant_id = &self.assistant_id;
    match tool_name {
        "record_knowledge" => operations::record_knowledge(self, args, assistant_id).await,
        "search_knowledge"  => queries::search_knowledge(self, args, assistant_id).await,
        "explore_context"   => queries::explore_context(self, args, assistant_id).await,
        "prune_knowledge"   => operations::prune_knowledge(self, args, assistant_id).await,
        _ => Err(format!("Tool {} not found", tool_name)),
    }
}

// After — resolve caller identity dynamically
async fn call_tool(&self, tool_name: &str, args: Value, session_id: Option<String>)
    -> Result<MCPResult, String>
{
    match tool_name {
        // Write tools: resolve caller's assistant_id for audit trail
        "record_knowledge" => {
            let caller_id = self.resolve_caller_id(&session_id).await?;
            operations::record_knowledge(self, args, &caller_id).await
        },
        // Read tools: global query (no assistant filter)
        "search_knowledge"  => queries::search_knowledge(self, args).await,
        "explore_context"   => queries::explore_context(self, args).await,
        // Delete tool: resolve caller's assistant_id for permission check
        "prune_knowledge"   => {
            let caller_id = self.resolve_caller_id(&session_id).await?;
            operations::prune_knowledge(self, args, &caller_id).await
        },
        _ => Err(format!("Tool {} not found", tool_name)),
    }
}

impl KnowledgeServer {
    /// Resolve the caller's assistant_id from session_id.
    /// Returns error if session not found or has no assistant config.
    async fn resolve_caller_id(&self, session_id: &Option<String>) -> Result<String, String> {
        let sid = session_id.as_ref().ok_or("No session_id provided")?;
        let session = crate::get_session_repository()
            .get_session(sid)
            .await
            .map_err(|e| format!("Session lookup failed: {e}"))?;
        let assistant_id = crate::agent::extract_assistant_id_from_session(&session)
            .ok_or("Session has no assistant configuration")?;
        Ok(assistant_id)
    }
}
```

**Design rationale:**

- `record_knowledge` stores with caller's `assistant_id` for audit trail (who saved this?)
- `prune_knowledge` uses caller's `assistant_id` for permission check (can only delete own chunks)
- `search_knowledge` / `explore_context` are **global read** — no assistant filter, all shared data visible

### 4.4 Repository — DRY Optional Filter Pattern

```rust
// Before — creates separate *_global functions (DRY violation)
async fn search_hybrid(&self, assistant_id: &str, ...) -> Result<...> {
    search_hybrid(&self.db, assistant_id, ...).await
}
async fn search_hybrid_global(&self, ...) -> Result<...> { ... }  // DRY violation

// After — single function with optional filter (DRY)
async fn search_hybrid(
    &self,
    assistant_id: Option<&str>,  // None = global, Some(aid) = filtered
    ...
) -> Result<...> {
    search_hybrid(&self.db, assistant_id, ...).await  // Repository handles Option internally
}
```

**Key changes per method:**

| Method                    | Before                 | After                          | Filter Behavior                    |
| ------------------------- | ---------------------- | ------------------------------ | ---------------------------------- |
| `search_hybrid`           | `assistant_id: &str`   | `assistant_id: Option<&str>`   | `None` = global, `Some` = filtered |
| `get_graph_context`       | `assistant_id: &str`   | `assistant_id: Option<&str>`   | Same                               |
| `delete_chunk`            | `assistant_id: &str`   | `assistant_id: Option<&str>`   | Same                               |
| `delete_chunks_atomic`    | `assistant_id: &str`   | `assistant_id: Option<&str>`   | Same                               |
| `find_existing_chunk_ids` | `assistant_id: &str`   | `assistant_id: Option<&str>`   | Same                               |
| `list_chunks`             | Already `Option<&str>` | Keep as-is                     | Already supports global            |
| `upsert_entity`           | `assistant_id: String` | `assistant_id: Option<String>` | Audit trail                        |
| `create_relationship`     | `assistant_id: String` | `assistant_id: Option<String>` | Audit trail                        |

**Repository internal filter logic (example):**

```rust
// search.rs internal — single source of truth
async fn search_hybrid(
    db: &DatabaseConnection,
    assistant_id: Option<&str>,
    ...
) -> Result<...> {
    let mut condition = Condition::all();
    if let Some(aid) = assistant_id {
        condition = condition.add(
            knowledge_chunk_v2::Column::AssistantId.eq(aid)
        );
    }
    // Continue with search logic using condition
}
```

### 4.5 `get_service_context()` — Actionable Hints

### 4.5 `get_service_context()` — Actionable Hints via SQLite `json_each`

```rust
// After
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let db = self.db.as_ref();

    // 1. Total chunk count (global)
    let total = knowledge_chunk_v2::Entity::find()
        .count(db)
        .await
        .unwrap_or(0);

    // 2. Top tags via SQLite json_each (single query, O(n) scan)
    let tags_sql = r#"
        SELECT tag, COUNT(*) as cnt
        FROM knowledge_chunks_v2,
             json_each(COALESCE(tags, '[]'))
        WHERE tags IS NOT NULL AND tags != '[]'
        GROUP BY tag
        ORDER BY cnt DESC
        LIMIT 5
    "#;
    let tags: Vec<(String, i64)> = sqlx::query_as::<_, (String, i64)>(tags_sql)
        .fetch_all(db)
        .await
        .map(|rows| rows.into_iter().map(|(tag, _)| tag).collect())
        .unwrap_or_default();

    // 3. Source labels (distinct, sorted)
    let sources_sql = r#"
        SELECT DISTINCT source
        FROM knowledge_chunks_v2
        WHERE source IS NOT NULL AND source != ''
        ORDER BY source
    "#;
    let sources: Vec<String> = sqlx::query_scalar::<_, String>(sources_sql)
        .fetch_all(db)
        .await
        .unwrap_or_default();

    ServiceContext::new(format!(
        "# Knowledge Base\n\nTotal Chunks: {}\n{}",
        total,
        if tags.is_empty() && sources.is_empty() {
            "No knowledge stored yet.".to_string()
        } else {
            format!(
                "{}{}",
                if sources.is_empty() {
                    String::new()
                } else {
                    format!("\nSources: {}", sources.join(", "))
                },
                if tags.is_empty() {
                    String::new()
                } else {
                    format!("\nTop Tags: {}", tags.join(", "))
                }
            )
        }
    ))
    .with_volatility(ContextVolatility::Medium)
}
```

**Why SQLite `json_each` instead of Rust aggregation:**

- `json_each` runs at the database level — no need to fetch all tags into Rust memory
- Single query replaces: fetch all chunks → iterate in Rust → count → sort
- The `COALESCE(tags, '[]')` handles NULL tags gracefully
- `LIMIT 5` keeps the prompt section bounded

**New output example:**

```
# Knowledge Base

Total Chunks: 42
Sources: conversation, https://example.com
Top Tags: tech, project_alpha, meeting, architecture
```

### 4.6 Helper Methods — Removed

The `get_top_tags()` and `list_sources()` helper methods from the original plan are **no longer needed**. The `get_service_context()` implementation now uses raw SQL queries directly via `sqlx::query_as` and `sqlx::query_scalar`, eliminating the need for separate aggregation methods.

All three pieces of information (total count, tags, sources) are generated in a single `get_service_context()` call:

```rust
// get_service_context() — self-contained, no helper dependencies
async fn get_service_context(&self, _options: Option<&Value>) -> ServiceContext {
    let db = self.db.as_ref();

    // 1. Total count — SeaORM
    let total = knowledge_chunk_v2::Entity::find().count(db).await.unwrap_or(0);

    // 2. Top tags — raw SQL via json_each
    let tags_sql = r#"SELECT tag, COUNT(*) as cnt FROM knowledge_chunks_v2,
             json_each(COALESCE(tags, '[]')) WHERE tags IS NOT NULL AND tags != '[]'
             GROUP BY tag ORDER BY cnt DESC LIMIT 5"#;
    let tags: Vec<String> = sqlx::query_as::<_, (String, i64)>(tags_sql)
        .fetch_all(db).await.map(|rows| rows.into_iter().map(|(tag, _)| tag).collect())
        .unwrap_or_default();

    // 3. Sources — raw SQL via DISTINCT
    let sources_sql = r#"SELECT DISTINCT source FROM knowledge_chunks_v2
             WHERE source IS NOT NULL AND source != '' ORDER BY source"#;
    let sources: Vec<String> = sqlx::query_scalar::<_, String>(sources_sql)
        .fetch_all(db).await.unwrap_or_default();

    // ... format and return ServiceContext ...
}
```

**Trade-off:** This couples `get_service_context()` to `sqlx` for the tag/source queries. The total count still uses SeaORM. This is intentional — `get_service_context()` is called once per turn, so raw SQL overhead is negligible, and the `json_each` aggregation is far more efficient than a Rust-side loop.

---

## 5. Expected System Prompt After Change

```
## Available Tools & Current State

## Planning
### Stable Context
- No goal set

### Live State
- Tasks: None

## Playbooks
Playbooks: None

## Knowledge Base

Total Chunks: 42
Sources: conversation, https://example.com
Top Tags: tech, project_alpha, meeting, architecture

## Attachments
### Live State
- No files available

## Workspace
### Live State
- Workspace Root: ...
```

---

## 6. Data Migration Strategy

### 6.1 Existing Data

All existing chunks have `assistant_id` populated. After migration, they remain in the database with their original `assistant_id` values. Queries simply stop filtering on it.

**No data migration needed.** The `assistant_id` column stays for reference (e.g., "who stored this?") but is no longer used for scoping.

### 6.2 New Chunks

New `record_knowledge` calls will store data with `assistant_id` set to the originating assistant's ID (for audit trail), but the `get_service_context()` query ignores it.

### 6.3 Prune Safety

`prune_knowledge` currently filters by `assistant_id`. After migration:

- **Option A (simpler):** Remove `assistant_id` filter — any assistant can delete any chunk
- **Option B (safer):** Keep `assistant_id` filter — assistants can only delete their own chunks

**Recommendation: Option B.** The tool signature changes to accept `Option<&str>` but defaults to filtering by the caller's assistant ID if provided.

---

## 7. Files to Change

| File                                                               | Change                                                                            |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------- |
| `src-tauri/src/mcp/builtin/knowledge/mod.rs`                       | Remove `assistant_id` field, update `get_service_context()`, update `call_tool()` |
| `src-tauri/src/mcp/builtin/knowledge/operations.rs`                | Remove `assistant_id` param from all functions                                    |
| `src-tauri/src/mcp/builtin/knowledge/queries.rs`                   | Remove `assistant_id` param from all functions                                    |
| `src-tauri/src/mcp/service_proxy/factory.rs`                       | Simplify Knowledge case — remove session lookup                                   |
| `src-tauri/src/repositories/knowledge_v2_repository/repository.rs` | Change params to `Option<&str>`, add global query variants                        |
| `src-tauri/src/repositories/knowledge_v2_repository/contracts.rs`  | Update trait signatures                                                           |
| `src-tauri/src/repositories/knowledge_v2_repository/search.rs`     | Add global search variant                                                         |
| `src-tauri/src/repositories/knowledge_v2_repository/graph.rs`      | Add global graph query variant                                                    |

---

## 8. Risks & Mitigations

| Risk                                                                           | Severity | Mitigation                                                                  |
| ------------------------------------------------------------------------------ | -------- | --------------------------------------------------------------------------- |
| Existing data with `assistant_id` becomes inaccessible if query logic is wrong | High     | Keep `Option<&str>` — default to `None` (global), test with explicit filter |
| Assistant A deletes Assistant B's knowledge                                    | Medium   | Keep `assistant_id` filter on `prune_knowledge` (Option B above)            |
| Prompt grows with hundreds of tags/sources                                     | Low      | Cap `get_top_tags()` at N=5, truncate sources list                          |
| Breaking change for any external callers of tool handlers                      | Low      | Internal only — no public API                                               |

---

## 9. Validation Checklist

- [ ] `KnowledgeServer::new()` compiles without `assistant_id`
- [ ] Factory creates Knowledge server without session lookup
- [ ] `get_service_context()` returns actionable hints (not assistant ID)
- [ ] `search_knowledge` returns results across all assistants
- [ ] `explore_context` traverses graph across all assistants
- [ ] `prune_knowledge` still filters by caller's `assistant_id` (Option B)
- [ ] `record_knowledge` stores with caller's `assistant_id` for audit
- [ ] `pnpm refactor:validate` passes
- [ ] System prompt includes `## Knowledge Base` section with correct content
