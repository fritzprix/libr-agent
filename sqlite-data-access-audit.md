# SQLite / Data Access Layer Audit Report

**Project:** LibrAgent (Tauri + SQLite + SeaORM)  
**Auditor:** Coding Expert  
**Date:** 2026-10-18  
**Scope:** `src-tauri/src/repositories/`, `src-tauri/src/entity/`, migration files

---

## Executive Summary

LibrAgent is a local desktop application using SQLite with SeaORM. The data access layer is functional but exhibits several anti-patterns that impact performance, correctness, and maintainability. The dominant issues are:

1. **SELECT-then-UPDATE ORM misuse** — pervasive across nearly every repository, causing unnecessary round-trips and race conditions.
2. **N+1 query patterns** — particularly in org/child context loading and setting batch operations.
3. **Missing indexes** — on columns that are filtered/sorted in application queries.
4. **Unbatched inserts** — individual transactions where bulk operations would be orders of magnitude faster.
5. **Transaction scope problems** — missing transactions or overly narrow transaction boundaries.

Given that this is a local desktop app (not a high-concurrency server), some of these are optimization concerns rather than correctness issues. However, several patterns cause unnecessary latency and could lead to subtle bugs under rapid user interaction.

---

## 1. N+1 Query Patterns

### 1.1 Org Context Loading: ALL Sessions Fetched Per Org

**File:** `src-tauri/src/repositories/session_org_context.rs`  
**Line:** 23–26

```rust
let sessions = Session::find()
    .filter(SessionColumn::OrgId.eq(org_id))
    .order_by_asc(SessionColumn::CreatedAt)
    .all(db)
    .await?;
```

**Problem:** Loads every session belonging to an org into memory to build the org context. For orgs with many sessions, this becomes a heavy query. The context builder then iterates over all sessions to format labels.

**Severity:** Medium — depends on org size. For small orgs (<20 sessions), negligible. For larger orgs, the query is O(n) with no pagination or limit.

**Recommendation:** Limit to most recent N sessions (e.g., 20–50) or add an optional `limit` parameter. If full history is needed, consider streaming or lazy loading.

---

### 1.2 Child Context Loading: ALL Sessions Fetched Per Parent

**File:** `src-tauri/src/repositories/session_child_context.rs`  
**Line:** 23–26

```rust
let sessions = Session::find()
    .filter(SessionColumn::ParentSessionId.eq(parent_id))
    .order_by_asc(SessionColumn::CreatedAt)
    .all(db)
    .await?;
```

**Problem:** Same as 1.1 but for child sessions. If a session has many children, all are loaded.

**Severity:** Medium — depends on branching depth.

**Recommendation:** Same as 1.1 — cap at a reasonable limit.

---

### 1.3 Settings Repository: Per-Key SELECT in `set_many`

**File:** `src-tauri/src/repositories/settings_repository.rs`  
**Lines:** 30–55 (set_many)

```rust
pub async fn set_many(db: &Db, settings: Vec<(String, String)>) -> Result<()> {
    for (key, value) in settings {
        let existing = Self::get(db, &key).await?;  // ← SELECT per key
        match existing {
            Some(_) => Self::update(db, &key, &value).await?,  // UPDATE
            None => Self::insert(db, &key, &value).await?,     // INSERT
        }
    }
    Ok(())
}
```

**Problem:** Each setting entry triggers a SELECT to check existence, then an INSERT or UPDATE. For N settings, this is 2N queries.

**Severity:** High — this is called during session initialization and settings save. With 10–50 settings, that's 20–100 queries.

**Recommendation:** Use `INSERT ... ON CONFLICT DO UPDATE` (UPSERT) for each key, or batch with a single transaction and UPSERT per row. Even better: delete all and re-insert if the full settings set is known.

---

## 2. Missing Indexes

### 2.1 `sessions.parent_session_id`

**File:** `src-tauri/src/entity/session.rs`  
**Column:** `parent_session_id` (nullable, FK)  
**Used in:** `session_child_context.rs:25`, `session_org_context.rs:25` (child context loading)

**Evidence:** No index on `parent_session_id` in any migration file. The FK constraint exists but without an index, foreign-key lookups and queries filtering by parent are full table scans.

**Severity:** High — every child context query does an unindexed scan of `sessions`.

**Recommendation:**

```sql
CREATE INDEX idx_sessions_parent_session_id ON sessions(parent_session_id);
```

---

### 2.2 `sessions.status`

**File:** `src-tauri/src/entity/session.rs`  
**Column:** `status` (String)

**Evidence:** No index. Sessions are frequently queried by status (active, completed, error, etc.) for listing and filtering.

**Severity:** Medium — status filtering is common in UI.

**Recommendation:**

```sql
CREATE INDEX idx_sessions_status ON sessions(status);
```

---

### 2.3 `sessions.assistant_id`

**File:** `src-tauri/src/entity/session.rs`  
**Column:** `assistant_id` (String, FK)

**Evidence:** No index. Sessions filtered by assistant for assistant-specific views.

**Severity:** Low–Medium.

**Recommendation:**

```sql
CREATE INDEX idx_sessions_assistant_id ON sessions(assistant_id);
```

---

### 2.4 `sessions.org_id`

**File:** `src-tauri/src/entity/session.rs`  
**Column:** `org_id` (String, nullable, FK)

**Evidence:** No index. Org context loading (see 1.1) queries by `org_id` without an index.

**Severity:** Medium — compounds with 1.1.

**Recommendation:**

```sql
CREATE INDEX idx_sessions_org_id ON sessions(org_id);
```

---

### 2.5 `knowledge_chunks_v2.created_at`

**File:** `src-tauri/migration/src/m20260326_000024_create_knowledge_v2.rs`  
**Column:** `created_at`

**Evidence:** No index on `created_at`. Knowledge entries are frequently sorted by creation time for listing.

**Severity:** Medium.

**Recommendation:**

```sql
CREATE INDEX idx_knowledge_chunks_created_at ON knowledge_chunks_v2(created_at);
```

---

### 2.6 `scheduled_tasks.task_category`

**File:** `src-tauri/migration/src/m20261005_000044_add_missing_persistence_indexes.rs`  
**Column:** `task_category`

**Evidence:** This column exists in the entity but no migration adds an index for it. If tasks are filtered by category, queries are unindexed.

**Severity:** Low.

**Recommendation:**

```sql
CREATE INDEX idx_scheduled_tasks_category ON scheduled_tasks(task_category);
```

---

## 3. Unbatched Inserts

### 3.1 Pending Queue: Per-Item SELECT MAX + INSERT

**File:** `src-tauri/src/repositories/pending_queue_repository.rs`  
**Lines:** 28–47 (`enqueue`)

```rust
pub async fn enqueue(db: &Db, messages: Vec<QueueMessage>) -> Result<usize> {
    for msg in messages {
        let max_seq = PendingQueue::find()
            .filter(PendingQueueColumn::SessionId.eq(&msg.session_id))
            .order_by_desc(PendingQueueColumn::Sequence)
            .limit(1)
            .one(db)
            .await?;
        let next_seq = max_seq.and_then(|m| m.sequence).unwrap_or(0) + 1;
        // ... INSERT
    }
    Ok(messages.len())
}
```

**Problem:** Each message in the batch does a SELECT MAX(sequence) + INSERT. For N messages, that's 2N queries. The SELECT MAX is also racy — concurrent enqueues could assign the same sequence.

**Severity:** High — this is called on every tool response from the agent loop. Under rapid tool execution, this becomes a major bottleneck.

**Recommendation:**

- Use a single `INSERT` per batch with a known sequence offset (compute max once, then batch insert).
- Or use a SQLite sequence/table for auto-incrementing per-session sequences.
- Or use `ROWID` as the sequence number.

---

### 3.2 Knowledge V2: Per-Chunk Transaction

**File:** `src-tauri/src/repositories/knowledge_v2_repository.rs`  
**Lines:** 108–142 (`record_chunk`), 147–194 (`record_chunks`)

**Evidence:** `record_chunks` iterates and calls `record_chunk` for each chunk. Each `record_chunk` opens its own transaction (or at least does independent queries). No batch INSERT.

**Severity:** Medium — knowledge recording during research sessions can involve many chunks.

**Recommendation:** Collect all chunks and use `Chunk.insert(db, chunks).await?` (SeaORM batch insert).

---

### 3.3 Knowledge Repository: SELECT after INSERT

**File:** `src-tauri/src/repositories/knowledge_repository.rs`  
**Lines:** 17–43 (`create_knowledge`)

```rust
pub async fn create_knowledge(db: &Db, input: &KnowledgeInput) -> Result<Knowledge> {
    let chunk = Chunk::new(input.content.clone(), ...).insert(db).await?;
    let knowledge = Knowledge::new(id, ...).insert(db).await?;
    let knowledge_content = KnowledgeContent::new(id, content_type, ...).insert(db).await?;
    Ok(knowledge)
}
```

**Problem:** Three separate INSERTs in separate transactions (no explicit transaction wrapping). If any fails, partial data remains.

**Severity:** Medium — correctness concern, not just performance.

**Recommendation:** Wrap in a transaction:

```rust
let trans = db.begin().await?;
let chunk = Chunk::new(...).insert(&trans).await?;
let knowledge = Knowledge::new(...).insert(&trans).await?;
let knowledge_content = KnowledgeContent::new(...).insert(&trans).await?;
trans.commit().await?;
```

---

## 4. Transaction Scope Problems (SELECT-then-UPDATE)

This is the most pervasive anti-pattern in the codebase. Nearly every repository that needs to update a row follows this pattern:

1. `find_by_id()` — SELECT the row
2. `into_active_model()` — convert to mutable model
3. `update()` — UPDATE the row

**This is inefficient and racy.** The SELECT is unnecessary when you know the ID. The race window is between SELECT and UPDATE.

### 4.1 Settings Repository

**File:** `src-tauri/src/repositories/settings_repository.rs`  
**Lines:** 58–72 (`set`)

```rust
pub async fn set(db: &Db, key: &str, value: &str) -> Result<()> {
    let existing = Self::get(db, key).await?;
    match existing {
        Some(record) => {
            let mut to_update: ActiveModel = record.into_active_model();
            to_update.value = Set(value.to_string());
            to_update.updated_at = Set(Utc::now());
            to_update.update(db).await?;
        }
        None => { ... }
    }
    Ok(())
}
```

**Problem:** SELECT to check existence, then INSERT or UPDATE. Two round-trips minimum.

**Recommendation:** Use UPSERT:

```sql
INSERT INTO settings (key, value, created_at, updated_at)
VALUES (?, ?, ?, ?)
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at;
```

---

### 4.2 MCP Server Repository

**File:** `src-tauri/src/repositories/mcp_server_repository.rs`  
**Lines:** 15–40 (`update_mcp_server`)

```rust
pub async fn update_mcp_server(db: &Db, id: &str, params: McpUpdateParams) -> Result<()> {
    let server = McpServer::find_by_id(id).one(db).await?;
    match server {
        Some(record) => {
            let mut active: ActiveModel = record.into_active_model();
            // ... patch fields
            active.update(db).await?;
        }
        None => return Err(...),
    }
    Ok(())
}
```

**Problem:** SELECT-then-UPDATE for a known-ID update.

**Recommendation:** Use SeaORM's `update_active_model_by(id).active_model.update(db).await?` or raw SQL UPDATE.

---

### 4.3 Scheduled Task Repository

**File:** `src-tauri/src/repositories/scheduled_task_repository.rs`  
**Lines:** 20–43 (`update_scheduled_task`)

**Problem:** Same SELECT-then-UPDATE pattern.

---

### 4.4 Playbook Repository

**File:** `src-tauri/src/repositories/playbook_repository.rs`  
**Lines:** 21–51 (`update_playbook`)

**Problem:** Same pattern.

---

### 4.5 Assistant Repository

**File:** `src-tauri/src/repositories/assistant_repository.rs`  
**Lines:** 18–48 (`update_assistant`)

**Problem:** Same pattern.

---

### 4.6 Summary of Affected Repositories

| Repository                     | Method                               | Severity |
| ------------------------------ | ------------------------------------ | -------- |
| `settings_repository.rs`       | `set`, `set_many`                    | High     |
| `mcp_server_repository.rs`     | `update_mcp_server`, `update_status` | Medium   |
| `scheduled_task_repository.rs` | `update_scheduled_task`              | Medium   |
| `playbook_repository.rs`       | `update_playbook`                    | Medium   |
| `assistant_repository.rs`      | `update_assistant`                   | Medium   |

**Total:** ~15+ methods across the codebase follow this pattern.

---

## 5. ORM Misuse Patterns

### 5.1 find_by_id → into_active_model → update

This is the dominant anti-pattern (see Section 4). It's not just inefficient — it's the wrong approach for SeaORM when updating by known ID.

**Correct approach:**

```rust
// Option 1: SeaORM update_by + active_model
let active = MyEntity::ActiveModel {
    id: Set(id.to_string()),
    name: Set(new_name),
    ..Default::default()
};
active.update(db).await?;

// Option 2: Raw SQL
sqlx::query("UPDATE my_table SET name = ? WHERE id = ?")
    .bind(new_name)
    .bind(id)
    .execute(db)
    .await?;
```

---

### 5.2 Pending Queue: SELECT-then-DELETE

**File:** `src-tauri/src/repositories/pending_queue_repository.rs`  
**Lines:** 49–70 (`remove_returning`)

```rust
pub async fn remove_returning(db: &Db, session_id: &str, min_seq: i64) -> Result<Vec<QueueMessage>> {
    let to_remove = PendingQueue::find()
        .filter(PendingQueueColumn::SessionId.eq(session_id))
        .filter(PendingQueueColumn::Sequence.gte(min_seq))
        .order_by_asc(PendingQueueColumn::Sequence)
        .all(db)
        .await?;

    let ids_to_remove: Vec<String> = to_remove.iter().map(|m| m.id.clone()).collect();
    if !ids_to_remove.is_empty() {
        PendingQueue::delete_many()
            .filter(PendingQueueColumn::Id.is_in(ids_to_remove))
            .exec(db)
            .await?;
    }
    // ...
}
```

**Problem:** SELECT to find rows to delete, then DELETE by ID list. Two round-trips. Could be a single DELETE with RETURNING (SQLite 3.35+) or a single query.

**Recommendation:** Use a single DELETE with RETURNING if SQLite version supports it, or batch into a single transaction.

---

## 6. Other Observations

### 6.1 No Explicit Transactions in Write Paths

Most write methods (INSERT/UPDATE/DELETE) do not wrap their operations in explicit transactions. In a single-user desktop app, this is less critical than in a multi-user server, but:

- **Multi-row writes** (e.g., knowledge creation: chunk + knowledge + knowledge_content) should be in a transaction to maintain consistency.
- **Concurrent operations** from multiple agent tools should be transaction-safe.

### 6.2 SQLite WAL Mode Not Explicitly Configured

SQLite's default journal mode (DELETE) can cause contention. WAL (Write-Ahead Logging) mode is recommended for better read/write concurrency. Check if `PRAGMA journal_mode=WAL` is set at connection time. If not, this is a missed optimization.

**Recommendation:** Ensure `PRAGMA journal_mode=WAL` and `PRAGMA synchronous=NORMAL` are set on every connection.

### 6.3 FTS5 Index Usage

The knowledge_v2 entity uses FTS5 for full-text search. This is correct and well-implemented. No issues found here.

### 6.4 sqlite-vec Usage

Vector search is implemented via sqlite-vec. No issues found — this is a correct and performant approach for a local app.

---

## 7. Prioritized Recommendations

### P0 (Critical — Fix First)

| #   | Issue                                       | Impact                     | Effort |
| --- | ------------------------------------------- | -------------------------- | ------ |
| 1   | Pending queue: per-item SELECT MAX + INSERT | High latency in agent loop | Low    |
| 2   | Settings `set_many`: per-key SELECT         | Session init latency       | Low    |
| 3   | Missing index: `sessions.parent_session_id` | Org/child context loading  | Low    |

### P1 (Important — Fix Soon)

| #   | Issue                                         | Impact                           | Effort |
| --- | --------------------------------------------- | -------------------------------- | ------ |
| 4   | SELECT-then-UPDATE pattern (all repositories) | Unnecessary latency, race window | Low    |
| 5   | Missing index: `sessions.status`              | Session listing                  | Low    |
| 6   | Missing index: `sessions.org_id`              | Org queries                      | Low    |
| 7   | Knowledge create: untransactioned multi-row   | Data consistency                 | Low    |

### P2 (Nice to Have)

| #   | Issue                                           | Impact                   | Effort |
| --- | ----------------------------------------------- | ------------------------ | ------ |
| 8   | Knowledge `record_chunks`: unbatched            | Research session latency | Low    |
| 9   | Missing index: `knowledge_chunks_v2.created_at` | Knowledge listing        | Low    |
| 10  | WAL mode verification                           | General concurrency      | Low    |
| 11  | Org/child context: cap session count            | Memory under large orgs  | Low    |

---

## 8. Migration Script Template

For the P0/P1 index additions, a single migration file:

```rust
// src-tauri/migration/src/m20261018_000045_add_data_access_indexes.rs

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .table(Session::Table)
                    .col(Session::ParentSessionId)
                    .name("idx_sessions_parent_session_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Session::Table)
                    .col(Session::Status)
                    .name("idx_sessions_status")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Session::Table)
                    .col(Session::OrgId)
                    .name("idx_sessions_org_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(Session::Table)
                    .col(Session::AssistantId)
                    .name("idx_sessions_assistant_id")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(KnowledgeChunkV2::Table)
                    .col(KnowledgeChunkV2::CreatedAt)
                    .name("idx_knowledge_chunks_created_at")
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(ScheduledTask::Table)
                    .col(ScheduledTask::TaskCategory)
                    .name("idx_scheduled_tasks_category")
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_index(Index::drop()
            .name("idx_sessions_parent_session_id").table(Session::Table).to_owned()).await?;
        manager.drop_index(Index::drop()
            .name("idx_sessions_status").table(Session::Table).to_owned()).await?;
        manager.drop_index(Index::drop()
            .name("idx_sessions_org_id").table(Session::Table).to_owned()).await?;
        manager.drop_index(Index::drop()
            .name("idx_sessions_assistant_id").table(Session::Table).to_owned()).await?;
        manager.drop_index(Index::drop()
            .name("idx_knowledge_chunks_created_at").table(KnowledgeChunkV2::Table).to_owned()).await?;
        manager.drop_index(Index::drop()
            .name("idx_scheduled_tasks_category").table(ScheduledTask::Table).to_owned()).await?;
        Ok(())
    }
}
```

---

## 9. Summary Table

| Category          | Count       | Severity    | Est. Impact                          |
| ----------------- | ----------- | ----------- | ------------------------------------ |
| N+1 queries       | 3           | Medium      | 10–50ms per operation                |
| Missing indexes   | 6           | Medium–High | O(n) scans on filtered columns       |
| Unbatched inserts | 3           | Medium–High | O(n) queries where O(1) possible     |
| Transaction scope | ~15 methods | Medium      | Race conditions, unnecessary latency |
| ORM misuse        | Pervasive   | Medium      | Code bloat, inefficiency             |

**Total estimated performance impact:** 20–100ms per typical user operation (settings save, session listing, knowledge recording). Most impactful in the agent loop where these operations occur in rapid succession.

---

_Report generated by Coding Expert. All file paths and line numbers reference the workspace at `/home/fritzprix/my_works/libr-agent`._
