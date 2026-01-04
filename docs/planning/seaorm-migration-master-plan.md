# SeaORM Migration Master Plan

**Project**: LibrAgent Built-in MCP Servers Database Migration  
**From**: Raw SQLx queries → **To**: SeaORM ORM  
**Status**: Planning Phase  
**Created**: January 4, 2026  
**Branch**: dev/0.4.0

---

## Executive Summary

This document outlines the complete migration strategy for replacing raw SQLx SQL queries with SeaORM ORM across all built-in MCP servers in LibrAgent. The migration will improve code maintainability, type safety, and enable easier database schema evolution.

**Scope**: 7 database tables across 3 modules (Planning, Playbook, Content Store)  
**Impact**: ~1,310 lines of code across 10 files  
**Estimated Duration**: 4-6 weeks (phased approach)

---

## Migration Phases

### Phase 0: Foundation & Setup (Week 1)

**Objective**: Establish SeaORM infrastructure and tooling

**Tasks**:

1. Add SeaORM dependencies to `Cargo.toml`
   - `sea-orm` (core ORM)
   - `sea-orm-migration` (migration framework)
   - `sea-query` (query builder)

2. Create migration infrastructure
   - Create `src-tauri/migration/` directory
   - Set up `Migrator` struct
   - Configure migration runner

3. Document migration patterns and conventions
   - Entity naming conventions
   - Migration file naming
   - Error handling patterns
   - Testing strategies

**Deliverables**:

- [ ] Updated `Cargo.toml` with SeaORM dependencies
- [ ] Migration infrastructure in `src-tauri/migration/`
- [ ] Developer guide: `docs/guides/seaorm-patterns.md`

**Success Criteria**:

- SeaORM compiles successfully
- Migration framework can run empty migrations
- All existing tests still pass

---

### Phase 1: Planning Module Migration (Week 2-3)

**Priority**: HIGH (Core feature, most complex)

**Objective**: Migrate Planning module's 3 tables and 6 files to SeaORM

#### 1.1 Schema & Entity Generation

**Tables**:

- `planning_goals` (5 columns)
- `planning_todos` (10 columns)
- `planning_scratchpad` (8 columns)

**Tasks**:

1. Generate SeaORM entities from existing schema
   - `src-tauri/entity/planning_goal.rs`
   - `src-tauri/entity/planning_todo.rs`
   - `src-tauri/entity/planning_scratchpad.rs`

2. Create initial migrations
   - `m20260104_000001_create_planning_tables.rs`
   - Preserve existing data (migration, not recreation)

3. Add indexes and foreign keys
   - Session ID indexes
   - Foreign key constraints to `sessions` table

**Deliverables**:

- [ ] 3 entity files in `src-tauri/entity/`
- [ ] 1 migration file for Planning tables
- [ ] Schema documentation updated

#### 1.2 Database Operations Migration

**Files to Refactor** (in order):

**1.2.1 `db.rs` (Schema Management)**

- **Current**: Custom `TableSchema` struct with `sync()` method
- **Replace with**: SeaORM migrations
- **Lines impacted**: ~180 lines
- **Complexity**: MEDIUM

**1.2.2 `goals.rs` (Goal Operations)**

- **Operations**: 4 queries (2 INSERT, 2 UPDATE)
- **Replace with**:
  - `planning_goal::ActiveModel::insert()`
  - `planning_goal::Entity::update_many()`
- **Lines impacted**: ~50 lines
- **Complexity**: LOW

**1.2.3 `scratchpad.rs` (Scratchpad Operations)**

- **Operations**: 7 queries (1 INSERT, 1 UPDATE, 3 SELECT, 1 DELETE, 1 duplicate check)
- **Replace with**: SeaORM Entity CRUD methods
- **Lines impacted**: ~150 lines
- **Complexity**: LOW-MEDIUM

**1.2.4 `context.rs` (Context Building)**

- **Operations**: 3 SELECT queries for fetching state
- **Replace with**:
  - `planning_goal::Entity::find().filter()`
  - `planning_todo::Entity::find().order_by()`
- **Lines impacted**: ~50 lines
- **Complexity**: LOW

**1.2.5 `todos.rs` (Todo Operations)**

- **Operations**: 12+ queries (complex hierarchy, bulk operations)
- **Replace with**:
  - `planning_todo::Entity` methods
  - Query builder for complex filters
  - `insert_many()` for subtasks
- **Lines impacted**: ~200 lines
- **Complexity**: HIGH (hierarchical data, parent-child relationships)

**1.2.6 `mod.rs` (Cleanup Operations)**

- **Operations**: 3 DELETE queries
- **Replace with**: `Entity::delete_many().filter()`
- **Lines impacted**: ~15 lines
- **Complexity**: LOW

**Deliverables**:

- [ ] Refactored `db.rs` (remove schema sync logic)
- [ ] Refactored `goals.rs` (SeaORM CRUD)
- [ ] Refactored `scratchpad.rs` (SeaORM CRUD)
- [ ] Refactored `context.rs` (SeaORM queries)
- [ ] Refactored `todos.rs` (SeaORM with query builder)
- [ ] Refactored `mod.rs` (SeaORM delete operations)
- [ ] Unit tests for all operations

#### 1.3 Testing & Validation

**Tasks**:

1. Create integration tests for Planning module
   - Test all CRUD operations
   - Test session isolation
   - Test hierarchical todo structure
   - Test concurrent operations

2. Validate existing functionality
   - Run all existing Planning tests
   - Test migration from old schema
   - Test data integrity

**Deliverables**:

- [ ] Integration test suite for Planning module
- [ ] Migration validation tests
- [ ] Performance benchmarks (before/after)

**Success Criteria**:

- All Planning tools work identically to SQLx version
- All tests pass (old + new)
- No data loss during migration
- Performance within 10% of SQLx baseline

---

### Phase 2: Playbook Module Migration (Week 4)

**Priority**: MEDIUM (Optional feature, moderate complexity)

**Objective**: Migrate Playbook module's 1 table and 2 files to SeaORM

#### 2.1 Schema & Entity Generation

**Table**:

- `playbooks` (8 columns, includes JSON fields)

**Tasks**:

1. Generate entity: `src-tauri/entity/playbook.rs`
   - Handle JSON serialization for `workflow` and `success_criteria`
   - Use `#[sea_orm(column_type = "Json")]` attribute

2. Create migration
   - `m20260104_000002_create_playbooks_table.rs`

**Deliverables**:

- [ ] Entity file: `playbook.rs`
- [ ] Migration file for Playbooks table

#### 2.2 Database Operations Migration

**2.2.1 `operations.rs` (Playbook CRUD)**

- **Operations**: 9+ queries including dynamic query building
- **Replace with**:
  - `playbook::ActiveModel` for CRUD
  - `QuerySelect` for dynamic filtering
  - JSON handling for workflow/criteria fields
- **Lines impacted**: ~250 lines
- **Complexity**: MEDIUM-HIGH (dynamic queries, JSON handling)

**Key Challenges**:

- Line 192: Dynamic query building based on filters
- JSON serialization for `workflow` and `success_criteria` fields
- Pagination support

**2.2.2 `types.rs` (Data Models)**

- **Current**: Custom `from_row()` deserializer
- **Replace with**: Remove method (SeaORM auto-handles)
- **Lines impacted**: ~15 lines
- **Complexity**: LOW

**Deliverables**:

- [ ] Refactored `operations.rs` (SeaORM + query builder)
- [ ] Refactored `types.rs` (remove from_row)
- [ ] Unit tests for all operations

#### 2.3 Testing & Validation

**Tasks**:

1. Test CRUD operations
2. Test JSON field serialization
3. Test dynamic query building
4. Test pagination

**Deliverables**:

- [ ] Integration test suite for Playbook module
- [ ] JSON serialization tests

**Success Criteria**:

- All Playbook tools work identically
- JSON fields serialize/deserialize correctly
- Dynamic queries return correct results
- All tests pass

---

### Phase 3: Content Store Module Migration (Week 5)

**Priority**: MEDIUM (Optional feature, moderate complexity)

**Objective**: Migrate Content Store module's 3 tables to SeaORM

#### 3.1 Schema & Entity Generation

**Tables**:

- `content_stores` (5 columns)
- `contents` (13 columns)
- `chunks` (5 columns)

**Tasks**:

1. Generate entities
   - `src-tauri/entity/content_store.rs`
   - `src-tauri/entity/content.rs`
   - `src-tauri/entity/content_chunk.rs`

2. Create migration
   - `m20260104_000003_create_content_store_tables.rs`
   - Handle existing `src_url` column addition

**Deliverables**:

- [ ] 3 entity files for Content Store
- [ ] Migration file with schema evolution logic

#### 3.2 Database Operations Migration

**3.2.1 `storage.rs` (Content Store Storage)**

- **Operations**: 10+ queries including bulk operations
- **Replace with**:
  - `Entity` methods for CRUD
  - `insert_many()` for bulk chunk insertion
  - Proper transaction handling for multi-table operations
- **Lines impacted**: ~300 lines
- **Complexity**: MEDIUM (bulk operations, transactions)

**Key Challenges**:

- Line 91: Connection setup (replace with SeaORM Database::connect)
- Line 389: Bulk chunk insertion (use `insert_many()`)
- Line 500-507: Cascading deletes (chunks → contents)

**3.2.2 `test_migration.rs` (Migration Tests)**

- **Replace with**: SeaORM migration testing utilities
- **Lines impacted**: ~100 lines
- **Complexity**: MEDIUM

**Deliverables**:

- [ ] Refactored `storage.rs` (SeaORM)
- [ ] Refactored test files
- [ ] Unit tests for storage operations

#### 3.3 Testing & Validation

**Tasks**:

1. Test bulk chunk operations
2. Test transaction rollback scenarios
3. Test schema migration from old versions
4. Test concurrent access patterns

**Deliverables**:

- [ ] Integration test suite
- [ ] Transaction tests
- [ ] Migration validation tests

**Success Criteria**:

- All Content Store operations work correctly
- Bulk operations maintain performance
- Transactions properly handle failures
- Schema migrations preserve data

---

### Phase 4: Integration & Cleanup (Week 6)

**Objective**: Finalize migration, optimize, and clean up

#### 4.1 Global Integration

**Tasks**:

1. Update database initialization in `src-tauri/src/lib.rs`
   - Replace SQLx connection with SeaORM Database
   - Run migrations on startup

2. Update MCP service initialization
   - Pass SeaORM DatabaseConnection instead of SqlitePool
   - Update all service constructors

3. Remove SQLx dependencies
   - Clean up `Cargo.toml` (remove sqlx)
   - Remove unused imports

**Deliverables**:

- [ ] Updated application initialization
- [ ] Updated service constructors
- [ ] Cleaned `Cargo.toml`

#### 4.2 Performance Optimization

**Tasks**:

1. Add connection pooling configuration
2. Optimize query patterns
3. Add query result caching where appropriate
4. Profile and benchmark critical paths

**Deliverables**:

- [ ] Performance tuning guide
- [ ] Benchmark results comparison
- [ ] Optimized query patterns

#### 4.3 Documentation

**Tasks**:

1. Update architecture documentation
2. Create SeaORM usage guide for contributors
3. Document migration patterns
4. Update README with new setup instructions

**Deliverables**:

- [ ] `docs/architecture/database-architecture.md`
- [ ] `docs/guides/seaorm-patterns.md`
- [ ] `docs/guides/adding-new-tables.md`
- [ ] Updated main README.md

#### 4.4 Final Testing

**Tasks**:

1. Run full test suite
2. Perform manual testing of all features
3. Test migration from production databases
4. Load testing and stress testing

**Deliverables**:

- [ ] Full test suite passing
- [ ] Manual test checklist completed
- [ ] Migration validation on real data
- [ ] Performance benchmarks

**Success Criteria**:

- All tests pass (unit + integration)
- All features work identically to pre-migration
- Performance meets or exceeds baseline
- Documentation is complete and accurate

---

## Risk Assessment & Mitigation

### High Risk Areas

**1. Data Loss During Migration**

- **Risk**: Existing user data could be lost or corrupted
- **Mitigation**:
  - Implement migration rollback mechanism
  - Test migrations on backup databases first
  - Add data validation after migration
  - Keep SQLx code in separate branch for rollback

**2. Breaking Changes in API**

- **Risk**: SeaORM might handle some operations differently
- **Mitigation**:
  - Extensive integration testing
  - Side-by-side comparison tests
  - Feature flags for gradual rollout

**3. Performance Regression**

- **Risk**: SeaORM might be slower for some operations
- **Mitigation**:
  - Benchmark before and after
  - Optimize query patterns
  - Use raw SQL for critical paths if needed

**4. Complex Query Translation**

- **Risk**: Some SQLx queries might be hard to translate
- **Mitigation**:
  - Use SeaORM's raw SQL escape hatch
  - Consult SeaORM documentation and examples
  - Consider refactoring query logic if needed

---

## Testing Strategy

### Test Levels

**1. Unit Tests**

- Test individual entity operations (CRUD)
- Test query builders
- Test data validation

**2. Integration Tests**

- Test multi-table operations
- Test transactions
- Test session isolation
- Test concurrent access

**3. Migration Tests**

- Test schema creation from scratch
- Test migration from old schema
- Test data preservation
- Test rollback scenarios

**4. Performance Tests**

- Benchmark CRUD operations
- Benchmark complex queries
- Benchmark bulk operations
- Compare with SQLx baseline

### Test Data

**Use Cases**:

- Empty database (new user)
- Small dataset (< 100 records)
- Medium dataset (< 10,000 records)
- Large dataset (> 100,000 records)
- Edge cases (special characters, very long text, null fields)

---

## Rollback Plan

### Rollback Triggers

- Critical bugs discovered in production
- Performance degradation > 30%
- Data loss or corruption detected
- Unresolvable migration issues

### Rollback Procedure

**1. Immediate Actions**

1. Stop deployment
2. Notify team
3. Assess impact

**2. Code Rollback**

1. Checkout SQLx branch: `git checkout dev/0.4.0-pre-seaorm`
2. Rebuild: `pnpm tauri build`
3. Deploy previous version

**3. Database Rollback**

1. Restore from pre-migration backup
2. Verify data integrity
3. Test application functionality

**4. Post-Rollback**

1. Analyze root cause
2. Fix issues in migration branch
3. Re-test before retry

---

## Dependencies & Prerequisites

### Technical Dependencies

- SeaORM 1.1.x (latest stable)
- SQLite 3.x
- Rust 1.75+
- Existing SQLx database schema

### Team Dependencies

- Code reviews for each phase
- QA testing sign-off
- Database backup procedures
- Deployment approval

---

## Success Metrics

### Code Quality

- [ ] 100% test coverage for new code
- [ ] Zero compiler warnings
- [ ] All clippy lints pass
- [ ] Code reviews approved

### Functionality

- [ ] All existing features work identically
- [ ] No regressions in functionality
- [ ] All integration tests pass
- [ ] Manual testing checklist complete

### Performance

- [ ] Query performance within 10% of baseline
- [ ] Memory usage stable or improved
- [ ] Startup time unchanged
- [ ] No new bottlenecks introduced

### Documentation

- [ ] Architecture docs updated
- [ ] API docs updated
- [ ] Migration guide complete
- [ ] README updated

---

## Timeline Summary

| Phase                    | Duration    | Key Milestone              |
| ------------------------ | ----------- | -------------------------- |
| Phase 0: Setup           | 1 week      | Infrastructure ready       |
| Phase 1: Planning Module | 2 weeks     | Core features migrated     |
| Phase 2: Playbook Module | 1 week      | Optional features migrated |
| Phase 3: Content Store   | 1 week      | All tables migrated        |
| Phase 4: Integration     | 1 week      | Production ready           |
| **Total**                | **6 weeks** | **Complete migration**     |

### Critical Path

Phase 0 → Phase 1 → Phase 4 (Minimum viable migration)  
Phases 2 & 3 can be done in parallel or deferred

---

## Approval & Sign-off

### Phase Approvals

- [ ] Phase 0: Infrastructure setup approved
- [ ] Phase 1: Planning module approved
- [ ] Phase 2: Playbook module approved
- [ ] Phase 3: Content Store approved
- [ ] Phase 4: Final integration approved

### Final Sign-off

- [ ] Technical lead approval
- [ ] QA sign-off
- [ ] Documentation complete
- [ ] Production deployment approved

---

## References

### SeaORM Documentation

- [SeaORM Book](https://www.sea-ql.org/SeaORM/)
- [Migration Guide](https://www.sea-ql.org/SeaORM/docs/migration/setting-up-migration/)
- [Entity Generation](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)

### LibrAgent Documentation

- [Architecture Guide](../architecture/chat-feature-architecture.md)
- [Coding Guidelines](.github/copilot-instructions.md)
- [Built-in Tools](../builtin-tools.md)

### Related Issues

- GitHub Issue: SeaORM Migration Tracking
- PR: dev/0.4.0 (#272)

---

**Document Owner**: Development Team  
**Last Updated**: January 4, 2026  
**Next Review**: End of Phase 1
