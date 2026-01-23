# Repository Pattern Refactoring Plan

**Status:** Draft  
**Created:** January 23, 2026  
**Target Version:** 0.5.0  
**Priority:** Medium

---

## Executive Summary

This refactoring plan addresses architectural inconsistency in database access patterns. Currently, only 3 out of 15 SQLite tables use the repository pattern, while the remaining 12 tables use direct SeaORM Entity access. This creates maintainability issues, testing challenges, and violates separation of concerns.

**Goal:** Implement repository pattern consistently across all database tables.

---

## Current State Analysis

### ✅ Tables WITH Repository Pattern (3/15)

| Table                       | Repository               | Location                                   | Status      |
| --------------------------- | ------------------------ | ------------------------------------------ | ----------- |
| `session`                   | `SessionRepository`      | `repositories/session_repository.rs`       | ✅ Complete |
| `message`                   | `MessageRepository`      | `repositories/message_repository.rs`       | ✅ Complete |
| `content`, `chunk`, `store` | `ContentStoreRepository` | `repositories/content_store_repository.rs` | ✅ Complete |

### ❌ Tables WITHOUT Repository Pattern (12/15)

| Table                 | Current Access Pattern               | Used In                                                                    |
| --------------------- | ------------------------------------ | -------------------------------------------------------------------------- | ----------- |
| `assistant`           | `AssistantRepository`                | `repositories/assistant_repository.rs`                                     | ✅ Complete |
| `playbook`            | `PlaybookRepository`                 | `repositories/playbook_repository.rs`                                      | ✅ Complete |
| `knowledge`           | Direct `knowledge::Entity`           | `mcp/builtin/knowledge/`                                                   |
| `planning_goal`       | Direct `planning_goal::Entity`       | `mcp/builtin/planning/goals.rs`                                            |
| `planning_todo`       | Direct `planning_todo::Entity`       | `mcp/builtin/planning/todos.rs`                                            |
| `planning_scratchpad` | Direct `planning_scratchpad::Entity` | `mcp/builtin/planning/scratchpad.rs`                                       |
| `settings`            | Direct `settings::Entity`            | `commands/settings_commands.rs`, `lib.rs`                                  |
| `mcp_server`          | Direct `mcp_server::Entity`          | `commands/mcp_server_config_commands.rs`, `service_proxy_manager.rs`       |
| `message_index_meta`  | Mixed (partial repository)           | `repositories/session_repository.rs`, `repositories/message_repository.rs` |

---

## Refactoring Scope

### Phase 1: Infrastructure & Simple Tables (Priority: High)

**Estimated Effort:** 3-4 days

1. **Settings Repository** (`settings` table)
   - **Complexity:** Low
   - **Impact:** Medium (used in multiple places)
   - **Files to create:**
     - `src-tauri/src/repositories/settings_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/commands/settings_commands.rs`
     - `src-tauri/src/lib.rs`
     - `src-tauri/src/mcp/builtin/workspace/utils.rs`
     - `src-tauri/src/state.rs` (add `get_settings_repository()`)
   - **Operations:**
     - `get_setting(key: &str) -> Option<Setting>`
     - `set_setting(key: &str, value: Value) -> Result<Setting, DbError>`
     - `delete_setting(key: &str) -> Result<(), DbError>`
     - `list_settings() -> Result<Vec<Setting>, DbError>`

2. **MCP Server Repository** (`mcp_server` table)
   - **Complexity:** Low
   - **Impact:** Medium
   - **Files to create:**
     - `src-tauri/src/repositories/mcp_server_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/commands/mcp_server_config_commands.rs`
     - `src-tauri/src/mcp/service_proxy_manager.rs`
     - `src-tauri/src/commands/agent_commands.rs`
     - `src-tauri/src/state.rs` (add `get_mcp_server_repository()`)
   - **Operations:**
     - `create_server(name: &str, config: Value) -> Result<MCPServer, DbError>`
     - `get_server(name: &str) -> Result<Option<MCPServer>, DbError>`
     - `update_server(name: &str, config: Value) -> Result<MCPServer, DbError>`
     - `delete_server(name: &str) -> Result<(), DbError>`
     - `list_servers() -> Result<Vec<MCPServer>, DbError>`

3. **Message Index Meta Repository** (complete existing partial implementation)
   - **Complexity:** Low
   - **Impact:** Low (internal use only)
   - **Files to create:**
     - None (extend `message_repository.rs`)
   - **Files to modify:**
     - `src-tauri/src/repositories/message_repository.rs`
     - `src-tauri/src/repositories/session_repository.rs`
   - **Operations:**
     - Move `delete_index_metadata()` from `SessionRepository` to `MessageRepository`
     - Consolidate all index metadata operations

---

### Phase 2: Assistant & Playbook (Priority: High) - ✅ Complete

**Estimated Effort:** 5-6 days

4. **Assistant Repository** (`assistant` table)
   - **Complexity:** Medium-High
   - **Impact:** High (core entity, used extensively)
   - **Files to create:**
     - `src-tauri/src/repositories/assistant_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/commands/assistant_crud_commands.rs`
     - `src-tauri/src/commands/agent_commands.rs`
     - `src-tauri/src/mcp/builtin/assistant/operations.rs`
     - `src-tauri/src/mcp/builtin/assistant/queries.rs`
     - `src-tauri/src/services/assistant_init.rs`
     - `src-tauri/src/state.rs` (add `get_assistant_repository()`)
   - **Operations:**
     - `create_assistant(data: CreateAssistantDto) -> Result<Assistant, DbError>`
     - `get_assistant(id: &str) -> Result<Option<Assistant>, DbError>`
     - `update_assistant(id: &str, data: UpdateAssistantDto) -> Result<Assistant, DbError>`
     - `delete_assistant(id: &str) -> Result<(), DbError>`
     - `list_assistants() -> Result<Vec<Assistant>, DbError>`
     - `search_assistants(query: &str) -> Result<Vec<Assistant>, DbError>`
     - `check_assistant_exists(name: &str) -> Result<bool, DbError>`
   - **Special Considerations:**
     - Handle assistant initialization logic
     - Preserve default assistant creation
     - Maintain cascade delete behavior for related entities

5. **Playbook Repository** (`playbook` table)
   - **Complexity:** Medium-High
   - **Impact:** High
   - **Files to create:**
     - `src-tauri/src/repositories/playbook_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/commands/playbook_commands.rs`
     - `src-tauri/src/commands/agent_commands.rs`
     - `src-tauri/src/mcp/builtin/playbook/operations.rs`
     - `src-tauri/src/mcp/integration_tests.rs`
     - `src-tauri/src/state.rs` (add `get_playbook_repository()`)
   - **Operations:**
     - `create_playbook(assistant_id: &str, data: CreatePlaybookDto) -> Result<Playbook, DbError>`
     - `get_playbook(id: i32, assistant_id: &str) -> Result<Option<Playbook>, DbError>`
     - `list_playbooks(assistant_id: &str, pagination: PaginationParams) -> Result<Page<Playbook>, DbError>`
     - `update_playbook(id: i32, assistant_id: &str, data: UpdatePlaybookDto) -> Result<Playbook, DbError>`
     - `delete_playbook(id: i32, assistant_id: &str) -> Result<(), DbError>`
     - `delete_by_assistant(assistant_id: &str) -> Result<(), DbError>`
     - `search_playbooks(assistant_id: &str, query: &str) -> Result<Vec<Playbook>, DbError>`
   - **Special Considerations:**
     - Composite primary key (id, assistant_id)
     - Pagination support
     - Sorting and filtering logic

---

### Phase 3: Builtin Tool Tables (Priority: Medium)

**Estimated Effort:** 7-8 days

6. **Knowledge Repository** (`knowledge` table)
   - **Complexity:** Medium
   - **Impact:** Medium (session-scoped)
   - **Files to create:**
     - `src-tauri/src/repositories/knowledge_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/mcp/builtin/knowledge/operations.rs`
     - `src-tauri/src/mcp/builtin/knowledge/queries.rs`
     - `src-tauri/src/mcp/builtin/knowledge/mod.rs`
     - `src-tauri/src/state.rs` (add `get_knowledge_repository()`)
   - **Operations:**
     - `save_knowledge(assistant_id: &str, key: &str, value: &str, metadata: Option<Value>) -> Result<Knowledge, DbError>`
     - `get_knowledge(assistant_id: &str, key: &str) -> Result<Option<Knowledge>, DbError>`
     - `search_knowledge(assistant_id: &str, query: &str, limit: usize) -> Result<Vec<Knowledge>, DbError>`
     - `list_knowledge(assistant_id: &str, pagination: PaginationParams) -> Result<Page<Knowledge>, DbError>`
     - `delete_knowledge(assistant_id: &str, key: &str) -> Result<(), DbError>`
     - `get_knowledge_count(assistant_id: &str) -> Result<u64, DbError>`
   - **Special Considerations:**
     - FTS5 full-text search integration
     - Assistant-scoped data isolation

7. **Planning Repository** (`planning_goal`, `planning_todo`, `planning_scratchpad` tables)
   - **Complexity:** High (3 related tables)
   - **Impact:** Medium
   - **Files to create:**
     - `src-tauri/src/repositories/planning_repository.rs`
   - **Files to modify:**
     - `src-tauri/src/mcp/builtin/planning/goals.rs`
     - `src-tauri/src/mcp/builtin/planning/todos.rs`
     - `src-tauri/src/mcp/builtin/planning/scratchpad.rs`
     - `src-tauri/src/mcp/builtin/planning/context.rs`
     - `src-tauri/src/state.rs` (add `get_planning_repository()`)
   - **Operations (Goals):**
     - `create_goal(session_id: &str, goal_text: &str) -> Result<PlanningGoal, DbError>`
     - `update_goal(session_id: &str, goal_text: &str) -> Result<PlanningGoal, DbError>`
     - `get_active_goal(session_id: &str) -> Result<Option<PlanningGoal>, DbError>`
     - `archive_goals(session_id: &str) -> Result<(), DbError>`
   - **Operations (Todos):**
     - `add_todo(session_id: &str, task: &str, priority: Option<i32>) -> Result<PlanningTodo, DbError>`
     - `get_todos(session_id: &str, filters: TodoFilters) -> Result<Vec<PlanningTodo>, DbError>`
     - `update_todo(id: i32, checked: bool, task: Option<&str>) -> Result<PlanningTodo, DbError>`
     - `delete_todo(id: i32) -> Result<(), DbError>`
     - `delete_todos_by_session(session_id: &str) -> Result<(), DbError>`
   - **Operations (Scratchpad):**
     - `get_scratchpad(session_id: &str) -> Result<Option<PlanningScratchpad>, DbError>`
     - `update_scratchpad(session_id: &str, content: &str) -> Result<PlanningScratchpad, DbError>`
     - `delete_scratchpad(session_id: &str) -> Result<(), DbError>`
   - **Special Considerations:**
     - Unified repository for related planning entities
     - Session-scoped data isolation
     - Atomic operations for goal status transitions

---

## Implementation Guidelines

### 1. Repository Structure Template

```rust
// src-tauri/src/repositories/{entity}_repository.rs
use super::error::DbError;
use async_trait::async_trait;
use sea_orm::{DatabaseConnection, EntityTrait, Set};
use crate::entity::{entity_name, entity_name::Entity as EntityModel};

/// {Entity} repository trait for abstraction and testability
#[async_trait]
pub trait {Entity}Repository: Send + Sync {
    /// Repository methods...
}

/// SQLite implementation of {Entity}Repository using SeaORM
#[derive(Debug, Clone)]
pub struct Sqlite{Entity}Repository {
    db: DatabaseConnection,
}

impl Sqlite{Entity}Repository {
    /// Create a new SQLite repository with the given database connection
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl {Entity}Repository for Sqlite{Entity}Repository {
    // Implementation...
}
```

### 2. State Management Integration

Update `src-tauri/src/state.rs`:

```rust
// Add repository getter
pub fn get_{entity}_repository() -> Arc<dyn {Entity}Repository> {
    let db = get_database_connection();
    Arc::new(Sqlite{Entity}Repository::new(db.clone()))
}
```

### 3. Commands Layer Migration

**Before:**

```rust
#[command]
pub async fn get_entity(id: String) -> Result<Entity, String> {
    let db = get_database_connection();
    entity::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("Failed: {}", e))
}
```

**After:**

```rust
#[command]
pub async fn get_entity(id: String) -> Result<Entity, String> {
    let repo = get_entity_repository();
    repo.get_entity(&id)
        .await
        .map_err(|e| e.to_string())
}
```

### 4. Builtin MCP Server Migration

**Before:**

```rust
// In mcp/builtin/{server}/operations.rs
KnowledgeEntity::find()
    .filter(knowledge::Column::AssistantId.eq(&assistant_id))
    .all(db)
    .await
```

**After:**

```rust
// In mcp/builtin/{server}/operations.rs
let repo = get_knowledge_repository();
repo.list_knowledge(&assistant_id, pagination)
    .await
```

### 5. Testing Requirements

For each repository:

1. **Unit Tests** (in repository module):

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;

       async fn setup_test_db() -> Sqlite{Entity}Repository {
           // In-memory SQLite setup with migrations
       }

       #[tokio::test]
       async fn test_create_and_get() { /* ... */ }

       #[tokio::test]
       async fn test_update() { /* ... */ }

       #[tokio::test]
       async fn test_delete() { /* ... */ }
   }
   ```

2. **Integration Tests** (in `commands/` layer):
   - Test repository through Tauri commands
   - Verify end-to-end data flow

---

## Migration Strategy

### Step-by-Step Process for Each Table

1. **Create Repository Files**
   - Implement trait definition
   - Implement SQLite concrete class
   - Add unit tests

2. **Update State Management**
   - Add repository getter to `state.rs`
   - Update `mod.rs` exports

3. **Migrate Commands Layer**
   - Replace direct Entity access with repository calls
   - Update error handling to use `DbError`
   - Verify commands compile

4. **Migrate Builtin Servers**
   - Replace direct Entity access in `mcp/builtin/`
   - Update service context queries
   - Maintain existing functionality

5. **Testing & Validation**
   - Run unit tests: `cargo test repositories::{entity}_repository`
   - Run integration tests: `cargo test commands::{entity}_commands`
   - Manual testing in application
   - Verify database operations via logs

6. **Code Quality Checks**
   - `cargo fmt` - Format Rust code
   - `cargo clippy` - Run linter
   - `pnpm refactor:validate` - Full validation pipeline

---

## Risk Assessment

### High Risk Areas

1. **Assistant Repository**
   - **Risk:** Used extensively; initialization logic complex
   - **Mitigation:** Implement first in isolation, extensive testing
   - **Rollback:** Keep original Entity access in comments initially

2. **Planning Repository**
   - **Risk:** Multiple related tables with complex state transitions
   - **Mitigation:** Atomic operations, transaction support
   - **Rollback:** Single table-at-a-time migration possible

3. **Builtin MCP Servers**
   - **Risk:** Session-scoped isolation must be maintained
   - **Mitigation:** Repository methods enforce session_id/assistant_id filters
   - **Rollback:** Servers can be reverted independently

### Low Risk Areas

- Settings Repository (simple CRUD, isolated usage)
- MCP Server Repository (simple CRUD, isolated usage)
- Message Index Meta (internal only)

---

## Success Criteria

### Code Quality

- [ ] All direct `Entity::find()` calls removed from commands layer
- [ ] All direct `Entity::find()` calls removed from builtin MCP servers
- [ ] Zero ESLint/Clippy warnings introduced
- [ ] All existing tests pass
- [ ] New repository unit tests achieve >80% coverage

### Functionality

- [ ] All CRUD operations work identically to current implementation
- [ ] Session/assistant-scoped data isolation maintained
- [ ] No performance degradation
- [ ] Error messages are clear and actionable

### Architecture

- [ ] Consistent repository pattern across all 15 tables
- [ ] Clean separation: Commands → Repositories → Entities
- [ ] State management provides dependency injection
- [ ] No direct database access outside repositories

---

## Timeline Estimate

| Phase                    | Tasks                            | Effort         | Duration    |
| ------------------------ | -------------------------------- | -------------- | ----------- |
| **Phase 1**              | Settings, MCP Server, Index Meta | 3-4 days       | Week 1      |
| **Phase 2**              | Assistant, Playbook              | 5-6 days       | Week 2      |
| **Phase 3**              | Knowledge, Planning              | 7-8 days       | Week 3-4    |
| **Testing & Validation** | Integration tests, manual QA     | 2-3 days       | Week 4      |
| **Documentation**        | Update architecture docs         | 1 day          | Week 4      |
| **Total**                |                                  | **18-22 days** | **4 weeks** |

---

## Post-Refactoring Benefits

1. **Maintainability**
   - Single source of truth for database operations
   - Easy to add caching, logging, or metrics
   - Centralized error handling

2. **Testability**
   - Mock repositories for unit testing
   - Isolated testing of business logic
   - In-memory test databases

3. **Consistency**
   - Uniform API across all tables
   - Standardized error handling
   - Predictable behavior

4. **Future-Proofing**
   - Easy database migration (SQLite → PostgreSQL)
   - Swap implementations without changing business logic
   - Repository decorators for cross-cutting concerns

---

## Appendix: File Checklist

### New Files to Create (7)

- [x] `src-tauri/src/repositories/settings_repository.rs`
- [x] `src-tauri/src/repositories/mcp_server_repository.rs`
- [x] `src-tauri/src/repositories/assistant_repository.rs`
- [x] `src-tauri/src/repositories/playbook_repository.rs`
- [ ] `src-tauri/src/repositories/knowledge_repository.rs`
- [ ] `src-tauri/src/repositories/planning_repository.rs`
- [ ] `docs/refactoring/repository-pattern-completion.md` (this file on completion)

### Files to Modify (~25-30)

**Repositories:**

- [x] `src-tauri/src/repositories/mod.rs` (add exports)
- [x] `src-tauri/src/repositories/message_repository.rs` (consolidate index meta)
- [x] `src-tauri/src/repositories/session_repository.rs` (remove index meta)

**State Management:**

- [x] `src-tauri/src/state.rs` (add repository getters)

**Commands:**

- [x] `src-tauri/src/commands/settings_commands.rs`
- [x] `src-tauri/src/commands/mcp_server_config_commands.rs`
- [x] `src-tauri/src/commands/assistant_crud_commands.rs`
- [x] `src-tauri/src/commands/playbook_commands.rs`
- [x] `src-tauri/src/commands/agent_commands.rs`

**Builtin MCP Servers:**

- [x] `src-tauri/src/mcp/builtin/assistant/operations.rs`
- [x] `src-tauri/src/mcp/builtin/assistant/queries.rs`
- [x] `src-tauri/src/mcp/builtin/assistant/mod.rs`
- [x] `src-tauri/src/mcp/builtin/playbook/operations.rs`
- [x] `src-tauri/src/mcp/builtin/playbook/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/knowledge/operations.rs`
- [ ] `src-tauri/src/mcp/builtin/knowledge/queries.rs`
- [ ] `src-tauri/src/mcp/builtin/knowledge/mod.rs`
- [ ] `src-tauri/src/mcp/builtin/planning/goals.rs`
- [ ] `src-tauri/src/mcp/builtin/planning/todos.rs`
- [ ] `src-tauri/src/mcp/builtin/planning/scratchpad.rs`
- [ ] `src-tauri/src/mcp/builtin/planning/context.rs`

**Other:**

- [ ] `src-tauri/src/lib.rs` (settings access)
- [x] `src-tauri/src/services/assistant_init.rs`
- [ ] `src-tauri/src/mcp/service_proxy_manager.rs`
- [x] `src-tauri/src/mcp/integration_tests.rs`

---

## Next Steps

1. **Review & Approval**
   - Team review of this plan
   - Prioritization confirmation
   - Timeline validation

2. **Phase 1 Kickoff**
   - Create feature branch: `feat/repository-pattern-phase1`
   - Implement Settings Repository
   - Implement MCP Server Repository
   - PR review and merge

3. **Continue with Phases 2 & 3**
   - Follow same branch/PR pattern
   - Incremental delivery per table
   - Continuous testing and validation

---

**Plan Version:** 1.0  
**Last Updated:** January 23, 2026  
**Next Review:** Upon Phase 1 completion
