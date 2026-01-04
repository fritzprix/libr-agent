# Refactoring Plan: SeaORM Migration Phase 4 - Integration & Cleanup

**Created**: January 5, 2026  
**Branch**: dev/0.4.0  
**Related**: [SeaORM Migration Master Plan](../planning/seaorm-migration-master-plan.md#phase-4-integration--cleanup-week-6)  
**Priority**: HIGH (Required to complete migration)  
**Estimated Effort**: 1 week

---

## 1. Objective

Finalize the SeaORM migration by integrating all migrated modules, optimizing performance, updating documentation, and cleaning up legacy SQLx dependencies. This phase ensures the application runs seamlessly with SeaORM as the sole ORM solution.

**Success Criteria**:
- Application initializes with SeaORM Database connection
- All builtin MCP servers use SeaORM DatabaseConnection
- SQLx dependency removed from Cargo.toml (except for migration crate)
- All tests pass (unit + integration)
- Performance meets or exceeds baseline
- Documentation reflects new architecture
- Zero compiler warnings or clippy issues

---

## 2. Current State / Problem Analysis

### 2.1 Architecture Overview

The application currently uses SQLx throughout the codebase with a global connection pool pattern. After Phases 1-3, only Planning, Playbook, and Content Store modules have been migrated to SeaORM, but the global initialization and dependency injection still rely on SQLx.

**Key Files Using SQLx** (from grep analysis):

```
src-tauri/src/
├── lib.rs                           # Application initialization (Lines 100-180) ⚠️
├── state.rs                         # Global state management (Lines 10, 23, 80, 93) ⚠️
├── repositories/
│   ├── session_repository.rs        # Session metadata (Lines 4, 85, 90, 237, 240) ⚠️
│   ├── message_repository.rs        # Message storage (Lines 4, 51, 56) ⚠️
│   └── content_store_repository.rs  # Content store refs (Lines 3, 16, 21) ⚠️
└── mcp/
    ├── service_proxy_manager.rs     # Session proxies (Lines 1, 40, 62, 97) ⚠️
    └── builtin/
        ├── planning/mod.rs          # Planning server (Line 13, 23, 28) ✅ MIGRATED
        ├── playbook/mod.rs          # Playbook server (Line 7, 21, 25) ✅ MIGRATED
        └── content_store/storage.rs # Content store (already SeaORM) ✅ MIGRATED
```

**⚠️ Requires Update** | **✅ Already Migrated**

### 2.2 Current Database Initialization Flow (lib.rs)

**Lines 80-180 in `src-tauri/src/lib.rs`**:

```rust
pub fn run_with_sqlite_sync(db_url: String) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    rt.block_on(async {
        // 1. SQLite connection pool setup (Lines 91-125)
        let pool = SqlitePoolOptions::new()
            .connect_with(options.clone())
            .await?;

        // 2. Repository initialization with SqlitePool (Lines 130-150)
        let message_repo = SqliteMessageRepository::new(pool.clone());
        message_repo.create_table().await.expect("...");
        
        let content_store_repo = SqliteContentStoreRepository::new(pool.clone());
        let session_repo = SqliteSessionRepository::new(pool.clone());
        session_repo.create_table().await.expect("...");

        // 3. Set global SQLite pool (Line 157)
        set_sqlite_pool(pool);

        // 4. Initialize MCP Manager with SQLite (Lines 165-175)
        let mcp_manager = MCPServerManager::new_with_session_manager_and_sqlite(
            session_manager_arc.clone(),
            db_url.clone(),
        ).await;

        set_mcp_manager(mcp_manager);

        // 5. Initialize Service Proxy Manager (Lines 177-183)
        let proxy_manager = MCPServiceProxyManager::new_from_static_refs();
        set_mcp_service_proxy_manager(proxy_manager);
    });

    run();
}
```

**Problems**:
1. **SQLite pool initialization**: Uses `SqlitePoolOptions` and `SqlitePool` type
2. **Manual table creation**: Calls `create_table()` on each repository
3. **Global pool storage**: Stores `SqlitePool` in static `OnceLock`
4. **Type coupling**: All downstream code depends on `SqlitePool` type

### 2.3 Current State Management (state.rs)

**Lines 1-100**:

```rust
use sqlx::sqlite::SqlitePool;

static SQLITE_POOL: OnceLock<SqlitePool> = OnceLock::new();

pub fn set_sqlite_pool(pool: SqlitePool) {
    SQLITE_POOL.set(pool).expect("SQLite pool already initialized");
}

pub fn get_sqlite_pool() -> &'static SqlitePool {
    SQLITE_POOL.get().expect("SQLite pool not initialized. Call set_sqlite_pool() first.")
}
```

**Problems**:
1. Hardcoded `SqlitePool` type throughout
2. All modules import `sqlx::sqlite::SqlitePool`
3. Static lifetime management with unsafe Arc construction in `MCPServiceProxyManager`

### 2.4 Repository Layer Analysis

**Three repositories currently using SQLx**:

| Repository | File | SqlitePool Usage | Table Creation | Complexity |
|------------|------|------------------|----------------|------------|
| `SqliteSessionRepository` | session_repository.rs | Lines 4, 85, 90, 237, 240 | `create_table()` Line 97-106 | MEDIUM |
| `SqliteMessageRepository` | message_repository.rs | Lines 4, 51, 56 | `create_table()` Line 63-92 | MEDIUM |
| `SqliteContentStoreRepository` | content_store_repository.rs | Lines 3, 16, 21 | None (legacy) | LOW |

**Sessions Table Schema** (Lines 100-112):
```sql
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    name TEXT,
    status TEXT NOT NULL DEFAULT 'idle',
    agent_config TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

**Messages Table Schema** (Lines 66-92):
```sql
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    tool_calls TEXT,
    tool_call_id TEXT,
    name TEXT,
    reasoning TEXT
);

CREATE TABLE IF NOT EXISTS message_index_meta (
    session_id TEXT PRIMARY KEY,
    last_indexed_message_id TEXT
);
```

**Challenge**: These repositories are used across the entire application and have manual schema management.

### 2.5 Builtin Server Initialization

**`src-tauri/src/mcp/builtin/mod.rs` Lines 350-410**:

```rust
pub async fn new_with_session_manager_and_sqlite(
    session_manager: Arc<SessionManager>,
    sqlite_db_url: String,
) -> Self {
    let mut registry = Self {
        servers: HashMap::new(),
    };

    // Register workspace server
    registry.register_server(Box::new(workspace::WorkspaceServer::new(
        "default".to_string(),
        session_manager.clone(),
    )));

    // Register content-store server with SQLite (Lines 390-398)
    let content_store_server =
        ContentStoreServer::new_with_sqlite(
            "default".to_string(),
            session_manager.clone(),
            sqlite_db_url,  // ⚠️ Raw URL, not DatabaseConnection
        )
        .await
        .expect("Failed to initialize content store with SQLite");

    registry.register_server(Box::new(content_store_server));

    registry
}
```

**Problems**:
1. Passes raw `sqlite_db_url` string instead of DatabaseConnection
2. Each server creates its own connection internally
3. No shared connection pool across servers
4. Planning and Playbook servers receive `Arc<SqlitePool>` in constructors

### 2.6 Service Proxy Manager

**`src-tauri/src/mcp/service_proxy_manager.rs` Lines 1-200**:

```rust
use sqlx::SqlitePool;

pub struct MCPServiceProxyManager {
    proxies: Arc<RwLock<HashMap<String, Arc<MCPServiceProxy>>>>,
    session_stdio_managers: Arc<RwLock<HashMap<String, SessionMCPManager>>>,
    session_http_managers: Arc<RwLock<HashMap<String, HttpSessionManager>>>,
    external_mcp_manager: Arc<MCPServerManager>,
    db_pool: Arc<SqlitePool>,  // ⚠️ SqlitePool type
    session_manager: Arc<SessionManager>,
    cleanup_task: Arc<Mutex<Option<JoinHandle<()>>>>,
    cleanup_shutdown: Arc<AtomicBool>,
    config: SessionIsolationConfig,
}

impl MCPServiceProxyManager {
    pub fn new(
        external_mcp_manager: Arc<MCPServerManager>,
        db_pool: Arc<SqlitePool>,  // ⚠️ SqlitePool type
        session_manager: Arc<SessionManager>,
    ) -> Self {
        // ...
    }

    pub fn new_from_static_refs() -> Self {
        // SAFETY: Creating Arc from 'static references (Lines 150-175)
        let pool_arc = unsafe {
            let ptr = get_sqlite_pool() as *const SqlitePool;
            let arc = Arc::<SqlitePool>::from_raw(ptr);
            let cloned = arc.clone();
            std::mem::forget(arc);
            cloned
        };

        Self::new(mcp_manager_arc, pool_arc, session_manager_arc)
    }
}
```

**Problems**:
1. `db_pool` field is `Arc<SqlitePool>`, needs to be `Arc<DatabaseConnection>`
2. Unsafe Arc creation from static reference for SQLite pool
3. All proxy instances receive SqlitePool reference

### 2.7 Current Cargo.toml Dependencies

**Lines 1-100 in `src-tauri/Cargo.toml`**:

```toml
[dependencies]
sqlx = { version = "0.7", default-features = false, features = ["runtime-tokio-rustls", "sqlite"] }
libsqlite3-sys = { version = "0.27", features = ["bundled"] }

# No SeaORM dependencies yet ⚠️
```

**After Phase 0-3**:
- SeaORM dependencies should be added
- SQLx should be removed (except for tests if needed)
- Migration framework included

---

## 3. Proposed Solution: SeaORM Integration

### 3.1 Migration Strategy Overview

**Four-step integration process**:

1. **Add SeaORM Core Migration Infrastructure** (if not done in Phase 0)
2. **Migrate Core Repositories** (Sessions, Messages)
3. **Update Global State & Initialization**
4. **Update Builtin Server Construction**
5. **Remove SQLx Dependencies**
6. **Performance Optimization & Documentation**

### 3.2 Step 1: Core Migration Infrastructure

**Target**: `src-tauri/migration/`

**Prerequisite**: Phase 0 should have created this, but verify:

```rust
// src-tauri/migration/mod.rs
use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260104_000001_create_planning_tables::Migration),  // Phase 1
            Box::new(m20260104_000002_create_playbooks_table::Migration),   // Phase 2
            Box::new(m20260105_000001_create_content_store_tables::Migration), // Phase 3
            Box::new(m20260105_000002_create_sessions_table::Migration),    // Phase 4
            Box::new(m20260105_000003_create_messages_tables::Migration),   // Phase 4
        ]
    }
}
```

### 3.3 Step 2: Create Session & Message Entities

**Entity 1: `src-tauri/entity/session.rs`**

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: Option<String>,
    pub status: String,  // "idle" | "busy" | "paused" | "error"
    pub agent_config: Option<String>,  // JSON string
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::message::Entity")]
    Messages,
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

**Entity 2: `src-tauri/entity/message.rs`**

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "messages")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub role: String,  // "user" | "assistant" | "system" | "tool"
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub timestamp: i64,
    pub tool_calls: Option<String>,  // JSON array
    pub tool_call_id: Option<String>,
    pub name: Option<String>,
    pub reasoning: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::session::Entity",
        from = "Column::SessionId",
        to = "super::session::Column::Id",
        on_delete = "Cascade"
    )]
    Session,
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

**Entity 3: `src-tauri/entity/message_index_meta.rs`**

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "message_index_meta")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub session_id: String,
    pub last_indexed_message_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
```

### 3.4 Step 3: Create Migration Files

**Migration 1: `m20260105_000002_create_sessions_table.rs`**

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::Name).string())
                    .col(ColumnDef::new(Sessions::Status).string().not_null().default("idle"))
                    .col(ColumnDef::new(Sessions::AgentConfig).string())
                    .col(ColumnDef::new(Sessions::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Sessions::UpdatedAt).big_integer().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Sessions::Table).to_owned()).await
    }
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
    Name,
    Status,
    AgentConfig,
    CreatedAt,
    UpdatedAt,
}
```

**Migration 2: `m20260105_000003_create_messages_tables.rs`**

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create messages table
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Messages::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Messages::SessionId).string().not_null())
                    .col(ColumnDef::new(Messages::Role).string().not_null())
                    .col(ColumnDef::new(Messages::Content).text().not_null())
                    .col(ColumnDef::new(Messages::Timestamp).big_integer().not_null())
                    .col(ColumnDef::new(Messages::ToolCalls).string())
                    .col(ColumnDef::new(Messages::ToolCallId).string())
                    .col(ColumnDef::new(Messages::Name).string())
                    .col(ColumnDef::new(Messages::Reasoning).string())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Messages::Table, Messages::SessionId)
                            .to(Sessions::Table, Sessions::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create message_index_meta table
        manager
            .create_table(
                Table::create()
                    .table(MessageIndexMeta::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(MessageIndexMeta::SessionId).string().not_null().primary_key())
                    .col(ColumnDef::new(MessageIndexMeta::LastIndexedMessageId).string())
                    .to_owned(),
            )
            .await?;

        // Create index on session_id for faster queries
        manager
            .create_index(
                Index::create()
                    .name("idx_messages_session_id")
                    .table(Messages::Table)
                    .col(Messages::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(MessageIndexMeta::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Messages::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Sessions {
    Table,
    Id,
}

#[derive(Iden)]
enum Messages {
    Table,
    Id,
    SessionId,
    Role,
    Content,
    Timestamp,
    ToolCalls,
    ToolCallId,
    Name,
    Reasoning,
}

#[derive(Iden)]
enum MessageIndexMeta {
    Table,
    SessionId,
    LastIndexedMessageId,
}
```

### 3.5 Step 4: Update Repositories to Use SeaORM

**File 1: `session_repository.rs`**

**Changes Required**:

1. Replace imports (Lines 1-10):
```rust
// BEFORE
use sqlx::{Row, SqlitePool};

// AFTER
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set, QueryFilter, ColumnTrait};
use crate::entity::{session, prelude::Session};
```

2. Update struct (Lines 85-90):
```rust
// BEFORE
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// AFTER
pub struct SqliteSessionRepository {
    db: DatabaseConnection,
}

impl SqliteSessionRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
```

3. Remove `create_table()` method (Lines 97-112):
```rust
// BEFORE
async fn create_table(&self) -> Result<(), DbError> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sessions (...)
    "#).execute(&self.pool).await?;
    Ok(())
}

// AFTER
async fn create_table(&self) -> Result<(), DbError> {
    // Table creation now handled by migrations
    // This method can be removed or made a no-op
    Ok(())
}
```

4. Update CRUD operations (Examples):
```rust
// BEFORE (upsert_session)
async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
    sqlx::query("INSERT INTO sessions (...) VALUES (...) ON CONFLICT(id) DO UPDATE SET ...")
        .bind(&session.id)
        .bind(&session.name)
        // ...
        .execute(&self.pool)
        .await?;
    Ok(())
}

// AFTER
async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
    let active_model = session::ActiveModel {
        id: Set(session.id.clone()),
        name: Set(session.name.clone()),
        status: Set(session.status.as_str().to_string()),
        agent_config: Set(session.agent_config.clone()),
        created_at: Set(session.created_at),
        updated_at: Set(session.updated_at),
    };

    session::Entity::insert(active_model)
        .on_conflict(
            sea_orm::sea_query::OnConflict::column(session::Column::Id)
                .update_columns([
                    session::Column::Name,
                    session::Column::Status,
                    session::Column::AgentConfig,
                    session::Column::UpdatedAt,
                ])
                .to_owned(),
        )
        .exec(&self.db)
        .await
        .map_err(|e| DbError::Database(e.to_string()))?;

    Ok(())
}
```

**File 2: `message_repository.rs`** - Similar pattern as sessions

### 3.6 Step 5: Update Global State Management

**File: `src-tauri/src/state.rs`**

**Changes Required**:

1. Update imports and type definitions (Lines 1-25):
```rust
// BEFORE
use sqlx::sqlite::SqlitePool;
static SQLITE_POOL: OnceLock<SqlitePool> = OnceLock::new();

// AFTER
use sea_orm::DatabaseConnection;
static DB_CONNECTION: OnceLock<DatabaseConnection> = OnceLock::new();
```

2. Update setter/getter functions (Lines 80-95):
```rust
// BEFORE
pub fn set_sqlite_pool(pool: SqlitePool) {
    SQLITE_POOL.set(pool).expect("SQLite pool already initialized");
}

pub fn get_sqlite_pool() -> &'static SqlitePool {
    SQLITE_POOL.get().expect("SQLite pool not initialized...")
}

// AFTER
pub fn set_database_connection(db: DatabaseConnection) {
    DB_CONNECTION.set(db).expect("Database connection already initialized");
}

pub fn get_database_connection() -> &'static DatabaseConnection {
    DB_CONNECTION.get().expect("Database connection not initialized. Call set_database_connection() first.")
}
```

3. Update repository constructors (Lines 100-150):
```rust
// BEFORE
pub fn set_message_repository(repo: SqliteMessageRepository) { ... }
pub fn set_session_repository(repo: SqliteSessionRepository) { ... }

// AFTER (same, but constructed differently in lib.rs)
```

### 3.7 Step 6: Update Application Initialization

**File: `src-tauri/src/lib.rs`**

**Changes Required** (Lines 80-180):

```rust
// BEFORE
pub fn run_with_sqlite_sync(db_url: String) {
    rt.block_on(async {
        // SQLite connection pool
        let pool = SqlitePoolOptions::new()
            .connect_with(options.clone())
            .await?;

        // Initialize repositories
        let message_repo = SqliteMessageRepository::new(pool.clone());
        message_repo.create_table().await.expect("...");
        
        let session_repo = SqliteSessionRepository::new(pool.clone());
        session_repo.create_table().await.expect("...");

        set_sqlite_pool(pool);

        // Initialize MCP Manager
        let mcp_manager = MCPServerManager::new_with_session_manager_and_sqlite(
            session_manager_arc.clone(),
            db_url.clone(),
        ).await;

        set_mcp_manager(mcp_manager);

        let proxy_manager = MCPServiceProxyManager::new_from_static_refs();
        set_mcp_service_proxy_manager(proxy_manager);
    });
}

// AFTER
pub fn run_with_sqlite_sync(db_url: String) {
    rt.block_on(async {
        // 1. Connect to database using SeaORM
        let db = sea_orm::Database::connect(&db_url)
            .await
            .expect("Failed to connect to database");

        println!("✅ Database connected: {db_url}");

        // 2. Run migrations
        use migration::{Migrator, MigratorTrait};
        Migrator::up(&db, None)
            .await
            .expect("Failed to run migrations");

        println!("✅ Database migrations applied");

        // 3. Initialize repositories with DatabaseConnection
        let message_repo = SqliteMessageRepository::new(db.clone());
        let content_store_repo = SqliteContentStoreRepository::new(db.clone());
        let session_repo = SqliteSessionRepository::new(db.clone());

        println!("✅ Repository instances initialized");

        // 4. Start background indexing worker
        let _indexing_worker = search::IndexingWorker::new(Duration::from_secs(300));
        println!("✅ Background message indexing worker started");

        // 5. Set global database connection
        set_database_connection(db.clone());
        println!("✅ Database connection initialized");

        // 6. Set global repository instances
        set_message_repository(message_repo);
        set_content_store_repository(content_store_repo);
        set_session_repository(session_repo);

        // 7. Initialize MCP Manager with DatabaseConnection
        let mcp_manager = MCPServerManager::new_with_session_manager_and_db(
            session_manager_arc.clone(),
            db.clone(),
        ).await;

        set_mcp_manager(mcp_manager);
        println!("✅ SeaORM-backed MCP Manager initialized");

        // 8. Initialize Service Proxy Manager
        let proxy_manager = MCPServiceProxyManager::new_from_static_refs();
        set_mcp_service_proxy_manager(proxy_manager);
        println!("✅ MCP Service Proxy Manager initialized");
    });

    run();
}
```

### 3.8 Step 7: Update MCPServerManager Construction

**File: `src-tauri/src/mcp/builtin/mod.rs`**

**Changes Required** (Lines 370-410):

```rust
// BEFORE
pub async fn new_with_session_manager_and_sqlite(
    session_manager: Arc<SessionManager>,
    sqlite_db_url: String,
) -> Self {
    // ...
    let content_store_server =
        ContentStoreServer::new_with_sqlite(
            "default".to_string(),
            session_manager.clone(),
            sqlite_db_url,  // ⚠️ Raw URL string
        ).await.expect("...");
    
    registry.register_server(Box::new(content_store_server));
    registry
}

// AFTER
pub async fn new_with_session_manager_and_db(
    session_manager: Arc<SessionManager>,
    db: sea_orm::DatabaseConnection,
) -> Self {
    let mut registry = Self {
        servers: HashMap::new(),
    };

    // Register workspace server (no DB needed)
    registry.register_server(Box::new(workspace::WorkspaceServer::new(
        "default".to_string(),
        session_manager.clone(),
    )));

    // Register content-store server with DatabaseConnection
    let content_store_server =
        ContentStoreServer::new_with_db(
            "default".to_string(),
            session_manager.clone(),
            db.clone(),  // ✅ DatabaseConnection
        ).await.expect("Failed to initialize content store");

    registry.register_server(Box::new(content_store_server));

    // Register MCP Manager server
    registry.register_server(Box::new(mcp_manager::MCPManagerServer::new()));

    registry
}
```

### 3.9 Step 8: Update Service Proxy Manager

**File: `src-tauri/src/mcp/service_proxy_manager.rs`**

**Changes Required**:

1. Update imports and struct (Lines 1-45):
```rust
// BEFORE
use sqlx::SqlitePool;

pub struct MCPServiceProxyManager {
    db_pool: Arc<SqlitePool>,
    // ...
}

// AFTER
use sea_orm::DatabaseConnection;

pub struct MCPServiceProxyManager {
    db: Arc<DatabaseConnection>,
    // ...
}
```

2. Update constructor (Lines 90-110):
```rust
// BEFORE
pub fn new(
    external_mcp_manager: Arc<MCPServerManager>,
    db_pool: Arc<SqlitePool>,
    session_manager: Arc<SessionManager>,
) -> Self {
    Self::new_with_config(external_mcp_manager, db_pool, session_manager, ...)
}

// AFTER
pub fn new(
    external_mcp_manager: Arc<MCPServerManager>,
    db: Arc<DatabaseConnection>,
    session_manager: Arc<SessionManager>,
) -> Self {
    Self::new_with_config(external_mcp_manager, db, session_manager, ...)
}
```

3. Update unsafe Arc construction (Lines 150-175):
```rust
// BEFORE
pub fn new_from_static_refs() -> Self {
    let pool_arc = unsafe {
        let ptr = get_sqlite_pool() as *const SqlitePool;
        let arc = Arc::<SqlitePool>::from_raw(ptr);
        let cloned = arc.clone();
        std::mem::forget(arc);
        cloned
    };

    Self::new(mcp_manager_arc, pool_arc, session_manager_arc)
}

// AFTER
pub fn new_from_static_refs() -> Self {
    let db_arc = unsafe {
        let ptr = get_database_connection() as *const DatabaseConnection;
        let arc = Arc::<DatabaseConnection>::from_raw(ptr);
        let cloned = arc.clone();
        std::mem::forget(arc);
        cloned
    };

    Self::new(mcp_manager_arc, db_arc, session_manager_arc)
}
```

4. Update proxy creation to pass DatabaseConnection (Lines 200-250):
```rust
// Pass db: Arc<DatabaseConnection> to builtin server constructors
// Planning, Playbook, Content Store servers now receive DatabaseConnection
```

### 3.10 Step 9: Update Builtin Server Constructors

**Planning Server** (`planning/mod.rs`):

```rust
// BEFORE
pub async fn new(session_id: String, db_pool: Arc<SqlitePool>) -> Result<Self, String> {
    db::sync_planning_tables(&db_pool).await?;
    Ok(Self { session_id, db_pool })
}

// AFTER
pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
    // Migration handles table creation, no need for sync_planning_tables
    Ok(Self { session_id, db })
}
```

**Playbook Server** (`playbook/mod.rs`):

```rust
// BEFORE
pub async fn new(session_id: String, db_pool: Arc<SqlitePool>) -> Result<Self, String> {
    operations::ensure_table_exists(&db_pool).await?;
    Ok(Self { session_id, db_pool })
}

// AFTER
pub async fn new(session_id: String, db: Arc<DatabaseConnection>) -> Result<Self, String> {
    // Migration handles table creation
    Ok(Self { session_id, db })
}
```

**Content Store Server** (`content_store/server.rs`):

```rust
// BEFORE
pub async fn new_with_sqlite(
    session_id: String,
    session_manager: Arc<SessionManager>,
    database_url: String,
) -> Result<Self, String> {
    let storage = storage::ContentStoreStorage::new_sqlite(database_url).await?;
    Ok(Self { session_id, session_manager, storage: Mutex::new(storage), ... })
}

// AFTER
pub async fn new_with_db(
    session_id: String,
    session_manager: Arc<SessionManager>,
    db: DatabaseConnection,
) -> Result<Self, String> {
    let storage = storage::ContentStoreStorage::new_with_db(db).await?;
    Ok(Self { session_id, session_manager, storage: Mutex::new(storage), ... })
}
```

### 3.11 Step 10: Update Cargo.toml

**File: `src-tauri/Cargo.toml`**

**Changes Required**:

```toml
# BEFORE
[dependencies]
sqlx = { version = "0.7", default-features = false, features = ["runtime-tokio-rustls", "sqlite"] }
libsqlite3-sys = { version = "0.27", features = ["bundled"] }

# AFTER
[dependencies]
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }

[dev-dependencies]
sea-orm-migration = "1.1"

# Remove sqlx and libsqlite3-sys from [dependencies]
# They may remain in [dev-dependencies] if needed for integration tests
```

### 3.12 Step 11: Update Entity Module

**File: `src-tauri/entity/mod.rs`** (Create if not exists)

```rust
pub mod session;
pub mod message;
pub mod message_index_meta;
pub mod planning_goal;       // Phase 1
pub mod planning_todo;        // Phase 1
pub mod planning_scratchpad;  // Phase 1
pub mod playbook;             // Phase 2
pub mod content_store;        // Phase 3
pub mod content;              // Phase 3
pub mod content_chunk;        // Phase 3

pub mod prelude {
    pub use super::session::Entity as Session;
    pub use super::message::Entity as Message;
    pub use super::message_index_meta::Entity as MessageIndexMeta;
    pub use super::planning_goal::Entity as PlanningGoal;
    pub use super::planning_todo::Entity as PlanningTodo;
    pub use super::planning_scratchpad::Entity as PlanningScratchpad;
    pub use super::playbook::Entity as Playbook;
    pub use super::content_store::Entity as ContentStore;
    pub use super::content::Entity as Content;
    pub use super::content_chunk::Entity as ContentChunk;
}
```

---

## 4. Reusable Code & Patterns

### 4.1 DatabaseConnection Conversion Helper

**Pattern**: Convert between SeaORM `Model` and internal repository structs

```rust
// Example for SessionMetadata
impl From<session::Model> for SessionMetadata {
    fn from(model: session::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            status: SessionStatus::from_str(&model.status).unwrap_or(SessionStatus::Idle),
            agent_config: model.agent_config,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<&SessionMetadata> for session::ActiveModel {
    fn from(meta: &SessionMetadata) -> Self {
        session::ActiveModel {
            id: Set(meta.id.clone()),
            name: Set(meta.name.clone()),
            status: Set(meta.status.as_str().to_string()),
            agent_config: Set(meta.agent_config.clone()),
            created_at: Set(meta.created_at),
            updated_at: Set(meta.updated_at),
        }
    }
}
```

### 4.2 Migration Execution Pattern

**Pattern**: Run migrations at startup with proper error handling

```rust
use migration::{Migrator, MigratorTrait};

async fn apply_migrations(db: &DatabaseConnection) -> Result<(), String> {
    match Migrator::up(db, None).await {
        Ok(_) => {
            println!("✅ Database migrations applied successfully");
            Ok(())
        }
        Err(e) => {
            eprintln!("❌ Migration failed: {e}");
            Err(format!("Failed to run migrations: {e}"))
        }
    }
}
```

### 4.3 Repository Initialization Pattern

**Pattern**: Initialize all repositories with shared DatabaseConnection

```rust
pub async fn initialize_repositories(
    db: DatabaseConnection,
) -> (SqliteMessageRepository, SqliteSessionRepository, SqliteContentStoreRepository) {
    let message_repo = SqliteMessageRepository::new(db.clone());
    let session_repo = SqliteSessionRepository::new(db.clone());
    let content_store_repo = SqliteContentStoreRepository::new(db.clone());

    (message_repo, session_repo, content_store_repo)
}
```

---

## 5. Testing & Validation Plan

### 5.1 Unit Tests

**New Tests to Add**:
- [ ] Test entity conversions (`From` trait implementations)
- [ ] Test repository operations with in-memory SQLite (`:memory:`)
- [ ] Test migration up/down operations
- [ ] Test DatabaseConnection cloning and sharing

**Existing Tests to Update**:
- [ ] Update all tests using `SqlitePool` to use `DatabaseConnection`
- [ ] Replace `create_table()` calls with migration runs
- [ ] Update test fixtures to use SeaORM entities

### 5.2 Integration Tests

**Test Scenarios**:
1. **Fresh Database**: Start app with empty database, run migrations, verify tables
2. **Existing Database**: Start app with SQLx-created database, verify migration works
3. **Multi-Session**: Create multiple sessions, verify isolation with DatabaseConnection
4. **Concurrent Access**: Test multiple proxies accessing database simultaneously
5. **Migration Rollback**: Test `Migrator::down()` properly removes tables

### 5.3 Performance Benchmarks

**Baseline vs. SeaORM**:

| Operation | Baseline (SQLx) | Target (SeaORM) | Method |
|-----------|-----------------|-----------------|--------|
| Session CRUD | TBD ms | ±5% | Single operations |
| Message insert (bulk) | TBD ms | ±10% | 100 messages |
| Session list (100 records) | TBD ms | ±5% | Query with pagination |
| Migration execution | N/A | < 1 second | First startup |
| Application startup | TBD ms | +100ms acceptable | Time to ready |

**Benchmark Tool**: Use `criterion` crate for micro-benchmarks

### 5.4 Manual Testing Checklist

- [ ] Start application with fresh database → No errors, migrations run
- [ ] Create new session → Session appears in database
- [ ] Send messages → Messages saved to database
- [ ] Switch between sessions → Correct data loaded
- [ ] Use Planning tools → Data persists across restarts
- [ ] Use Playbook tools → Data persists across restarts
- [ ] Use Content Store → Data persists across restarts
- [ ] Restart application → All data preserved
- [ ] Check logs for SQLx warnings → None found

---

## 6. Migration Rollback Plan

### 6.1 Rollback Triggers
- Critical bug in repository layer
- Performance degradation > 20%
- Data corruption detected
- Migration failures on production databases
- Unresolvable SeaORM API issues

### 6.2 Rollback Procedure

**Step 1**: Stop deployment immediately

**Step 2**: Code rollback
```bash
git checkout dev/0.4.0-pre-phase4
pnpm tauri build
```

**Step 3**: Database rollback
- If migrations were applied, run `Migrator::down()` to revert schema
- Or restore from pre-migration backup

**Step 4**: Verify functionality
- Run full test suite
- Manual testing of all features

---

## 7. Dependencies & Prerequisites

### 7.1 Phase Dependencies

**Required**:
- [x] Phase 0: SeaORM infrastructure setup
- [x] Phase 1: Planning module migrated
- [x] Phase 2: Playbook module migrated
- [x] Phase 3: Content Store module migrated

**Optional** (can proceed without):
- [ ] Performance benchmarks from Phases 1-3
- [ ] Documentation from Phases 1-3

### 7.2 Cargo.toml Updates

```toml
[dependencies]
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }

[build-dependencies]
# No changes needed

[dev-dependencies]
sea-orm-migration = "1.1"
# Optional: keep sqlx for legacy integration tests if needed
```

---

## 8. Timeline & Milestones

| Task | Duration | Dependencies | Deliverables |
|------|----------|--------------|--------------|
| **8.1** Create Session/Message Entities | 0.5 days | Phase 0 | 3 entity files |
| **8.2** Create Migration Files | 0.5 days | 8.1 complete | 2 migration files |
| **8.3** Update Repositories | 1 day | 8.2 complete | Updated session_repository.rs, message_repository.rs |
| **8.4** Update State Management | 0.5 days | 8.3 complete | Updated state.rs |
| **8.5** Update Application Init | 1 day | 8.4 complete | Updated lib.rs, mcp/builtin/mod.rs |
| **8.6** Update Service Proxy Manager | 0.5 days | 8.5 complete | Updated service_proxy_manager.rs |
| **8.7** Update Builtin Servers | 0.5 days | 8.6 complete | Updated planning, playbook, content_store |
| **8.8** Remove SQLx Dependencies | 0.5 days | 8.7 complete | Updated Cargo.toml, removed imports |
| **8.9** Testing & Validation | 1 day | 8.8 complete | All tests passing |
| **8.10** Documentation | 0.5 days | 8.9 complete | Updated docs |
| **Total** | **6.5 days** | | **Production ready** |

**Buffer**: Add 20% buffer → **8 days total (1.6 weeks)**

---

## 9. Success Criteria

### 9.1 Functional Requirements
- [x] Application starts successfully with SeaORM
- [x] Migrations run automatically on first startup
- [x] All repository operations work identically to SQLx version
- [x] Session isolation maintained across all features
- [x] No data loss during migration
- [x] All existing features work unchanged

### 9.2 Performance Requirements
- [x] Startup time increase < 100ms
- [x] Query performance within 10% of SQLx baseline
- [x] Memory usage stable or improved
- [x] No new bottlenecks introduced

### 9.3 Code Quality Requirements
- [x] Zero compiler warnings
- [x] All clippy lints pass
- [x] 100% test coverage for new code
- [x] Code reviews approved
- [x] No `any` types or unsafe code (except Arc construction if necessary)

### 9.4 Documentation Requirements
- [x] Architecture docs updated
- [x] Migration guide complete
- [x] API documentation updated
- [x] README reflects new setup

---

## 10. Clarification Q-List

### Q1: Migration Strategy for Existing Databases
**Question**: How should we handle users with existing SQLx databases?

**Options**:
- A) Auto-migrate on first startup (detect SQLx schema, run migrations)
- B) Require manual migration with CLI tool
- C) Preserve SQLx schema, add SeaORM tables alongside (dual mode)

**Recommendation**: Option A - Auto-migration with backup prompt

**Implementation**:
```rust
async fn smart_migration(db_url: &str) -> Result<(), String> {
    let db = Database::connect(db_url).await?;
    
    // Detect if tables exist
    let has_tables = check_existing_tables(&db).await?;
    
    if has_tables {
        println!("🔄 Existing database detected, running migrations...");
        // Backup recommended
    }
    
    Migrator::up(&db, None).await?;
    Ok(())
}
```

**Impact**: MEDIUM - Requires careful testing with production databases

---

### Q2: DatabaseConnection Cloning Cost
**Question**: Is cloning DatabaseConnection expensive? Should we use Arc everywhere?

**Options**:
- A) Clone DatabaseConnection freely (it's cheap, uses Arc internally)
- B) Always wrap in Arc<DatabaseConnection>
- C) Use &DatabaseConnection references everywhere

**Recommendation**: Option A - Clone freely

**Rationale**: SeaORM's `DatabaseConnection` is already an Arc internally, so cloning is cheap (just increments reference count).

**Impact**: LOW - No performance concern

---

### Q3: create_table() Method Removal
**Question**: Should we keep `create_table()` methods in repositories for backwards compatibility?

**Options**:
- A) Remove immediately (migration handles schema)
- B) Keep as no-op for backwards compatibility
- C) Deprecate with warning, remove in next major version

**Recommendation**: Option B - Keep as no-op

**Rationale**: Some tests or external code might call these methods.

```rust
async fn create_table(&self) -> Result<(), DbError> {
    // No-op: Schema managed by migrations
    log::debug!("create_table() called but schema is now managed by migrations");
    Ok(())
}
```

**Impact**: LOW - Maintains API compatibility

---

### Q4: Error Type Consistency
**Question**: Should we standardize error types across repositories?

**Options**:
- A) Keep current `DbError` enum, adapt for SeaORM errors
- B) Create new `SeaOrmError` type
- C) Use `anyhow::Error` throughout

**Recommendation**: Option A - Adapt existing `DbError`

**Rationale**: Minimize API changes, easier migration.

```rust
impl From<sea_orm::DbErr> for DbError {
    fn from(err: sea_orm::DbErr) -> Self {
        DbError::Database(err.to_string())
    }
}
```

**Impact**: LOW - Transparent to consumers

---

### Q5: Performance Monitoring
**Question**: How should we monitor performance degradation after migration?

**Options**:
- A) Add telemetry to repository methods (log timing)
- B) Manual benchmarking before/after
- C) Use profiling tools (flamegraphs)

**Recommendation**: Option A + Option B - Combined approach

**Implementation**:
```rust
async fn upsert_session(&self, session: &SessionMetadata) -> Result<(), DbError> {
    let start = std::time::Instant::now();
    let result = /* ... SeaORM operation ... */;
    log::debug!("upsert_session took {:?}", start.elapsed());
    result
}
```

**Impact**: LOW - Easy to add/remove, useful for debugging

---

### Q6: Unsafe Arc Construction Safety
**Question**: Is the unsafe Arc construction in `new_from_static_refs()` safe long-term?

**Options**:
- A) Keep current unsafe pattern (works, well-documented)
- B) Refactor to use `once_cell::sync::Lazy` instead
- C) Pass DatabaseConnection explicitly instead of static ref

**Recommendation**: Option B - Refactor to Lazy

**Rationale**: Eliminates unsafe code, more idiomatic Rust.

```rust
use once_cell::sync::Lazy;

static DB_CONNECTION_ARC: Lazy<Arc<DatabaseConnection>> = Lazy::new(|| {
    Arc::new(get_database_connection().clone())
});

pub fn new_from_static_refs() -> Self {
    Self::new(
        mcp_manager_arc,
        DB_CONNECTION_ARC.clone(),
        session_manager_arc,
    )
}
```

**Impact**: MEDIUM - Safer, but requires refactoring

---

## 11. References

### 11.1 Related Documentation
- [SeaORM Migration Master Plan](../planning/seaorm-migration-master-plan.md)
- [Refactoring Plan Submission Guide](../../refactoring_plan_submission_guide.md)
- [SeaORM Book](https://www.sea-ql.org/SeaORM/)
- [SeaORM Migration Guide](https://www.sea-ql.org/SeaORM/docs/migration/setting-up-migration/)
- [LibrAgent Architecture](../architecture/chat-feature-architecture.md)

### 11.2 Related Code Files

**Primary Targets**:
- `src-tauri/src/lib.rs` (Lines 80-180)
- `src-tauri/src/state.rs` (Lines 1-100)
- `src-tauri/src/repositories/session_repository.rs`
- `src-tauri/src/repositories/message_repository.rs`
- `src-tauri/src/mcp/service_proxy_manager.rs`
- `src-tauri/src/mcp/builtin/mod.rs` (Lines 350-410)

**Entity Files**:
- `src-tauri/entity/session.rs` (to create)
- `src-tauri/entity/message.rs` (to create)
- `src-tauri/entity/message_index_meta.rs` (to create)

**Migration Files**:
- `src-tauri/migration/m20260105_000002_create_sessions_table.rs` (to create)
- `src-tauri/migration/m20260105_000003_create_messages_tables.rs` (to create)

### 11.3 Related Issues & PRs
- GitHub PR: dev/0.4.0 (#272)
- Master Plan: SeaORM Migration Phases 0-3 (prerequisites)

---

**Document Status**: ✅ READY FOR REVIEW  
**Next Step**: Team review and approval before implementation  
**Approval Required**: Technical Lead, Database Specialist, Senior Developer

---

**End of Refactoring Plan**
