# Phase 1 Repository Pattern Migration - Status Report

**Generated:** January 23, 2026  
**Branch:** dev/0.4.0

## Executive Summary

Phase 1 repository pattern migration has **15 remaining external violations** across 3 tables. While repository implementations are complete, some commands and service code still use direct Entity access instead of repositories.

## Tables Status

### ✅ Fully Migrated (2/5)

- **settings** - Zero violations ✓
- **message_index_meta** - Zero violations ✓

### ⚠️ Partially Complete (3/5)

- **mcp_server** - 1 violation
- **message** - 4 violations
- **session** - 10 violations

## Detailed Violations

### 1. mcp_server (1 violation)

**File:** `src-tauri/src/mcp/builtin/assistant/operations.rs:2`

```rust
use crate::entity::{
    assistant, assistant::Entity as AssistantEntity,
    mcp_server::Entity as McpServerEntity,  // ❌ Direct Entity import
};
```

**Fix:** This is just an import for type reference. Should verify if actually used or can be removed.

---

### 2. message (4 violations)

#### 2a. `src-tauri/src/commands/messages_commands.rs:195`

```rust
// ❌ Direct Entity query
let messages = crate::entity::message::Entity::find()
    .filter(crate::entity::message::Column::SessionId.eq(session_id))
    .order_by_desc(crate::entity::message::Column::CreatedAt)
    .limit(max_docs as u64)
    .all(db)
    .await
```

**Fix:** Add `get_messages_by_session()` method to MessageRepository

#### 2b. `src-tauri/src/commands/messages_commands.rs:287`

```rust
// ❌ Direct Entity query for global search
let messages = crate::entity::message::Entity::find()
    .order_by_desc(crate::entity::message::Column::CreatedAt)
    .limit(max_docs as u64)
    .all(db)
    .await
```

**Fix:** Add `get_recent_messages()` method to MessageRepository

#### 2c. `src-tauri/src/search/background_worker.rs:82`

```rust
// ❌ Direct Entity query for distinct sessions
let sessions: Vec<String> = crate::entity::message::Entity::find()
    .select_only()
    .column(crate::entity::message::Column::SessionId)
    .distinct()
    .into_tuple()
    .all(db)
    .await
```

**Fix:** Add `get_distinct_sessions()` method to MessageRepository

#### 2d. `src-tauri/src/search/background_worker.rs:124`

```rust
// ❌ Duplicate of 2a - same pattern
let messages = crate::entity::message::Entity::find()
    .filter(crate::entity::message::Column::SessionId.eq(session_id))
    .order_by_desc(crate::entity::message::Column::CreatedAt)
    .limit(max_docs as u64)
    .all(db)
    .await
```

**Fix:** Use MessageRepository method from 2a

---

### 3. session (10 violations)

#### 3a. `src-tauri/src/commands/playbook_commands.rs:47`

```rust
// ❌ Direct Entity query for session lookup
let session_model = session::Entity::find_by_id(session_id)
    .one(db)
    .await
```

**Fix:** Use SessionRepository `get()` method

#### 3b-3c. Test Setup Code (2 violations)

- `src-tauri/src/mcp/builtin/knowledge/mod.rs:314` - Schema creation
- `src-tauri/src/mcp/service_proxy_manager.rs:582` - Schema creation

**Fix:** These are test/setup utilities. May be acceptable or need repository test helpers.

#### 3d-3h. Session Insert Operations (5 violations)

- `src-tauri/src/mcp/builtin/knowledge/mod.rs:351`
- `src-tauri/src/mcp/service_proxy_manager.rs:639`
- `src-tauri/src/mcp/service_proxy_manager.rs:711`
- `src-tauri/src/mcp/service_proxy_manager.rs:888`
- `src-tauri/src/mcp/service_proxy_manager.rs:995`

```rust
// ❌ Direct Entity insert
session::Entity::insert(new_session)
    .exec(db)
    .await
```

**Fix:** Use SessionRepository `create()` or `upsert()` method

#### 3i-3j. Entity Import References (2 violations)

- `src-tauri/src/mcp/builtin/playbook/mod.rs:1`
- `src-tauri/src/mcp/service_proxy.rs:482`

```rust
use crate::entity::session::Entity as SessionEntity;
```

**Fix:** Remove imports if not actually used, or use repository instead

---

## Completion Criteria

To complete Phase 1, all external code must:

1. ✅ **Stop using Entity::find()** - Use repository query methods
2. ✅ **Stop using Entity::insert()** - Use repository create/upsert methods
3. ✅ **Stop importing Entity types** - Use repository interfaces
4. ✅ **Route all DB access through repositories** - No direct SeaORM calls

## Recommended Actions

### Priority 1: Message Repository Extensions

Add these methods to `MessageRepository`:

- `get_messages_by_session(session_id, limit) -> Vec<Message>`
- `get_recent_messages(limit) -> Vec<Message>`
- `get_distinct_sessions() -> Vec<String>`

### Priority 2: Session Repository Usage

Update service code to use existing `SessionRepository` methods:

- Replace `Entity::find_by_id()` with `repository.get()`
- Replace `Entity::insert()` with `repository.create()` or `repository.upsert()`

### Priority 3: Cleanup

- Remove unnecessary Entity imports
- Verify test/setup code patterns

## Timeline Estimate

- Message repository extensions: 30 minutes
- Session repository migration: 45 minutes
- Testing and verification: 30 minutes

**Total:** ~2 hours to complete Phase 1

## Testing Strategy

After fixes:

1. Run `.\scripts\check-phase1-completion.ps1` - Should show 0 violations
2. Run `pnpm refactor:validate` - Should pass all checks
3. Run Rust unit tests - Should pass with new repository methods
4. Manual testing - Verify message search and session operations work

---

**Status:** Phase 1 is 90% complete. Repository infrastructure is solid; just need to update remaining call sites to use repositories instead of direct Entity access.
