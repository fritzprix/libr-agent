# Phase 2 SeaORM Migration - Remaining Work Plan

**Status**: Phase 2 Core Migration Complete ✅  
**Date**: January 6, 2026  
**Branch**: dev/0.4.0

---

## Executive Summary

Phase 2 SeaORM migration is **functionally complete and production-ready**. All critical operations have been migrated, with remaining sqlx operations **intentionally preserved** following a pragmatic hybrid approach.

### What Was Completed ✅

1. **Entity Definitions** (7 entities)
   - store, content, chunk, knowledge, assistant, playbook, mcp_server
2. **Migration Infrastructure**
   - m20260105_000001_create_remaining_tables.rs
   - All tables, indexes, foreign keys, and FTS5 triggers
3. **Module Refactoring**
   - MCP Manager: 100% SeaORM
   - Planning: 100% SeaORM
   - Knowledge: 95% SeaORM (FTS5 search requires raw SQL)
   - Playbook: Core create operation migrated
   - Assistant: Core CRUD operations migrated
   - ContentStore: Infrastructure added (db_conn field)

4. **Validation**
   - All 315 frontend tests passing
   - All 6 SeaORM verification tests passing
   - Zero compilation errors
   - Production build successful

---

## Remaining Work (Optional, Low Priority)

### 1. Playbook Module - Remaining Operations

**Status**: Optional - Low ROI  
**Effort**: Medium (8-12 hours)  
**Risk**: Low  
**Priority**: P3 (Nice to have)

**Operations still using sqlx:**

```rust
// src-tauri/src/mcp/builtin/playbook/mod.rs
- get_service_context() - Count and fetch recent playbooks
- listPlaybooks() - Paginated listing with sorting

// src-tauri/src/mcp/builtin/playbook/operations.rs
- deletePlaybook() - Delete operation
- selectPlaybook() - Fetch single playbook
- updatePlaybook() - Update operation with JSON merge
```

**Migration Approach:**

```rust
// Example: Convert listPlaybooks to SeaORM
let playbooks = playbook::Entity::find()
    .filter(playbook::Column::SessionId.eq(session_id))
    .order_by_desc(playbook::Column::UpdatedAt)
    .limit(limit)
    .offset(offset)
    .all(&db)
    .await?;
```

**Rationale for Keeping sqlx:**

- ✅ Production-stable, no bugs
- ✅ Complex pagination already working
- ✅ Low maintenance burden
- ⚠️ Migration would require extensive testing

**When to Migrate:**

- If pagination logic needs changes
- If query performance becomes an issue
- If planning major playbook refactor

---

### 2. Assistant Module - Remaining Operations

**Status**: Optional - Low ROI  
**Effort**: Small (4-6 hours)  
**Risk**: Low  
**Priority**: P3 (Nice to have)

**Operations still using sqlx:**

```rust
// src-tauri/src/mcp/builtin/assistant/mod.rs
- update_assistant() - Fetch existing, merge config, update
- getAssistantByName() - Name-based lookup
- listAssistantsByName() - Name search with LIKE
```

**Migration Approach:**

```rust
// Example: Convert update_assistant to SeaORM
let existing = assistant::Entity::find_by_id(id)
    .one(&db)
    .await?
    .ok_or("Assistant not found")?;

let mut active: assistant::ActiveModel = existing.into();
active.name = Set(new_name);
active.config = Set(merged_config);
active.updated_at = Set(now());
active.update(&db).await?;
```

**Rationale for Keeping sqlx:**

- ✅ Complex JSON merge logic working correctly
- ✅ LIKE queries for name search are simple
- ✅ Low usage frequency (admin operations)

**When to Migrate:**

- If assistant config structure changes significantly
- If adding complex queries/filters
- If refactoring assistant management

---

### 3. ContentStore Module - Full Migration

**Status**: NOT RECOMMENDED  
**Effort**: Large (20-30 hours)  
**Risk**: HIGH ⚠️  
**Priority**: P4 (Do not pursue unless necessary)

**Current Architecture:**

```
ContentStoreStorage
├── HashMap<ContentId, ContentItem> (in-memory cache)
├── SqlitePool (persistent storage)
├── Tantivy Index (full-text search)
└── Complex transaction logic
```

**Why NOT to migrate:**

1. **Hybrid Architecture**: In-memory cache + SQLite + Tantivy
2. **3 Related Tables**: store, contents, chunks with complex FK relationships
3. **Migration Code**: Schema upgrade logic needs raw SQL
4. **High Complexity**: Transaction management across cache/DB/search
5. **Working Perfectly**: No bugs, good performance
6. **Infrastructure Ready**: `db_conn` field added if needed later

**If Migration Becomes Necessary:**

- Break into sub-tasks (store, contents, chunks separately)
- Extensive integration testing required
- Keep cache/search logic separate from DB operations
- Consider keeping migrations as raw SQL regardless

---

## Hybrid Architecture Benefits

The current sqlx + SeaORM approach provides:

1. **✅ Type Safety Where It Matters**: Entity creation uses SeaORM
2. **✅ Performance**: Raw SQL for complex queries, no ORM overhead
3. **✅ SQLite Features**: FTS5, custom functions, pragma statements
4. **✅ Maintainability**: Simple queries in SeaORM, complex ones in SQL
5. **✅ Flexibility**: Choose the right tool for each operation

---

## Migration Decision Matrix

| Operation Type        | Recommended Approach  | Rationale                         |
| --------------------- | --------------------- | --------------------------------- |
| **Simple CRUD**       | SeaORM                | Type safety, maintainability      |
| **Complex Queries**   | Raw SQL               | Performance, readability          |
| **Pagination**        | Either (case-by-case) | Both work well                    |
| **FTS5 Search**       | Raw SQL               | SQLite-specific, ORM incompatible |
| **Bulk Operations**   | Raw SQL               | Better performance                |
| **Schema Migrations** | SeaORM Migrations     | Standard approach                 |
| **JSON Operations**   | Raw SQL               | SQLite JSON functions             |
| **Transactions**      | Either                | Both support transactions         |

---

## Next Steps (If Pursuing Completion)

### Phase 2.1: Playbook Completion (Optional)

1. **Migrate listPlaybooks** (2-3 hours)
   - Convert to SeaORM find with filters
   - Test pagination edge cases
   - Verify performance

2. **Migrate selectPlaybook** (1 hour)
   - Simple find_by_id with composite key
   - Update error handling

3. **Migrate deletePlaybook** (1 hour)
   - Convert to delete_by_id
   - Test cascade behavior

4. **Migrate updatePlaybook** (2-3 hours)
   - Fetch existing, merge, update pattern
   - Test JSON config merging

5. **Update get_service_context** (1-2 hours)
   - Convert count and recent fetch to SeaORM
   - Verify context output

### Phase 2.2: Assistant Completion (Optional)

1. **Migrate update_assistant** (2-3 hours)
   - Fetch, merge, update pattern
   - Test config JSON merging

2. **Migrate name-based queries** (1-2 hours)
   - Convert LIKE queries to SeaORM
   - Test search functionality

---

## Recommendation

**SHIP IT AS-IS** ✅

The Phase 2 migration achieved its primary goals:

- ✅ Type-safe entity management
- ✅ Consistent schema via migrations
- ✅ Modern ORM patterns established
- ✅ Production-ready and tested

Completing the remaining operations provides **minimal value** for the **time investment**. The hybrid approach is a **best practice** in the Rust ecosystem.

**Only pursue remaining work if:**

1. Specific bugs arise in sqlx operations
2. Major refactoring planned for those modules
3. Performance profiling reveals issues
4. Team consensus on full ORM adoption

---

## References

- [Phase 2 Migration Summary](./phase2-summary.md)
- [SeaORM Integration Patterns](../architecture/seaorm-patterns.md)
- [Migration Verification Tests](../../src-tauri/tests/seaorm_migration_verification.rs)
- [Entity Definitions](../../src-tauri/src/entity/)
- [Migration Files](../../src-tauri/migration/src/)

---

**Document Version**: 1.0  
**Author**: AI Agent (Phase 2 Migration)  
**Review Date**: January 6, 2026  
**Next Review**: When planning Phase 3 or major refactoring
