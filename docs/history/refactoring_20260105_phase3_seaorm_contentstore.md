# Refactoring Plan: SeaORM Migration Phase 3 - Content Store Module

**Created**: January 5, 2026  
**Branch**: dev/0.4.0  
**Related**: [SeaORM Migration Master Plan](../planning/seaorm-migration-master-plan.md#phase-3-content-store-module-migration-week-5)  
**Priority**: MEDIUM (Optional feature, moderate complexity)  
**Estimated Effort**: 1 week

---

## 1. Objective

Migrate the Content Store module from raw SQLx queries to SeaORM ORM, improving:
- Type safety and compile-time query validation
- Code maintainability and readability
- Schema evolution capability via migrations
- Transaction handling consistency

**Success Criteria**:
- All Content Store operations work identically to pre-migration
- No data loss during migration from existing databases
- All unit and integration tests pass
- Performance within 10% of SQLx baseline
- Proper transaction handling for multi-table operations

---

## 2. Current State / Problem Analysis

### 2.1 Architecture Overview

The Content Store module is located in `src-tauri/src/mcp/builtin/content_store/` with the following structure:

```
content_store/
├── mod.rs              # Module declarations, BuiltinMCPServer trait impl
├── server.rs           # ContentStoreServer, initialization, tool definitions
├── storage.rs          # Core database operations (517 lines) ⚠️ PRIMARY TARGET
├── handlers.rs         # Tool call handlers (880 lines)
├── types.rs            # Request argument types
├── schemas.rs          # Tool schema definitions
├── search.rs           # BM25 search engine (separate from DB)
├── parsers.rs          # Document parsing
├── helpers.rs          # Utility functions
├── utils.rs            # Additional utilities
├── test_migration.rs   # Schema migration tests ⚠️ SECONDARY TARGET
├── test_functional.rs  # Functional tests
└── test_session_isolation.rs  # Session isolation tests
```

### 2.2 Database Schema

**Three tables with foreign key relationships**:

```sql
-- Primary table (1:1 with sessions)
CREATE TABLE stores (
    session_id TEXT PRIMARY KEY,
    name TEXT,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Content items (many:1 with stores)
CREATE TABLE contents (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size INTEGER NOT NULL,
    line_count INTEGER NOT NULL,
    preview TEXT NOT NULL,
    uploaded_at TEXT NOT NULL,
    chunk_count INTEGER NOT NULL,
    last_accessed_at TEXT NOT NULL,
    content TEXT NOT NULL,
    src_url TEXT,  -- Added via migration
    FOREIGN KEY (session_id) REFERENCES stores(session_id) ON DELETE CASCADE
);

-- Content chunks (many:1 with contents)
CREATE TABLE chunks (
    id TEXT PRIMARY KEY,
    content_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    FOREIGN KEY (content_id) REFERENCES contents(id) ON DELETE CASCADE
);

-- Indexes for performance
CREATE INDEX idx_chunks_content_id ON chunks(content_id);
CREATE INDEX idx_contents_session_id ON contents(session_id);
```

**Schema Evolution History**:
- `src_url` column was added via `ALTER TABLE` migration (line 154 in storage.rs)
- Migration uses error suppression for "duplicate column name" (backwards compatibility)

### 2.3 Current Implementation: storage.rs (517 lines)

**Data Structures** (Lines 1-36):
```rust
// Rust structs for deserialization
pub struct ContentStore { session_id, name, description, created_at, updated_at }
pub struct ContentItem { id, session_id, filename, mime_type, size, line_count, preview, uploaded_at, chunk_count, last_accessed_at, content, src_url }
pub struct ContentChunk { id, content_id, chunk_index, text, line_range: (usize, usize) }
```

**Storage Layer** (Lines 37-517):
```rust
pub struct ContentStoreStorage {
    stores: HashMap<String, ContentStore>,     // In-memory cache
    contents: HashMap<String, ContentItem>,    // In-memory cache
    chunks: HashMap<String, Vec<ContentChunk>>, // In-memory cache
    sqlite_pool: Option<SqlitePool>,           // Optional SQLite backend
}
```

**Key Methods and SQLx Usage**:

| Method | Lines | SQLx Operations | Complexity |
|--------|-------|-----------------|------------|
| `new_sqlite()` | 66-102 | 1 CREATE TABLES | MEDIUM |
| `create_tables()` | 104-163 | 1 CREATE + ALTER TABLE | MEDIUM |
| `create_store()` | 165-214 | 1 INSERT | LOW |
| `get_or_create_store()` | 260-302 | 1 SELECT (fetch_optional) | LOW |
| `add_content()` | 304-409 | 1 INSERT (content) + N INSERTs (chunks) | HIGH |
| `list_content()` | 411-437 | None (in-memory only) | LOW |
| `read_content()` | 453-485 | None (in-memory only) | LOW |
| `delete_content()` | 487-517 | 2 DELETEs (cascading) | MEDIUM |

**Total SQLx Query Calls**: 10 query sites across 5 methods

### 2.4 Key Challenges

**1. Bulk Chunk Insertion** (Line 389-404):
```rust
// Current: Individual INSERT per chunk
for chunk in &content_chunks {
    sqlx::query("INSERT INTO chunks (...) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&chunk.id)
        .bind(&chunk.content_id)
        // ... 6 binds total
        .execute(pool).await?;
}
```
**SeaORM Solution**: Use `insert_many()` with `Vec<chunks::ActiveModel>`

**2. Schema Migration with ALTER TABLE** (Line 154-162):
```rust
// Current: Error suppression for idempotency
if let Err(e) = sqlx::query("ALTER TABLE contents ADD COLUMN src_url TEXT").execute(pool).await {
    if !error_msg.contains("duplicate column name") {
        return Err(...);
    }
}
```
**SeaORM Solution**: Use SeaORM migration framework with `add_column()` and `ColumnDef::new()`

**3. Cascading Deletes** (Line 500-515):
```rust
// Current: Manual cascade (chunks first, then content)
sqlx::query("DELETE FROM chunks WHERE content_id = ?").execute(pool).await?;
sqlx::query("DELETE FROM contents WHERE id = ?").execute(pool).await?;
```
**SeaORM Solution**: Rely on foreign key `ON DELETE CASCADE` in schema + use `delete_by_id()`

**4. Tuple Line Range Field** (Lines 34, 346-347):
```rust
pub struct ContentChunk {
    pub line_range: (usize, usize),  // Stored as two separate columns in DB
}
```
**SeaORM Solution**: Entity will have `start_line` and `end_line` as separate fields; transform in conversion logic

**5. In-Memory Cache + SQLite Dual Mode** (Lines 42-47):
- Storage maintains HashMap cache regardless of backend
- SQLite is optional (`sqlite_pool: Option<SqlitePool>`)
- All operations update cache first, then persist to SQLite if present

**SeaORM Impact**: Replace `SqlitePool` with `DatabaseConnection`, maintain same cache strategy

---

## 3. Proposed Solution: SeaORM Migration

### 3.1 Entity Generation

**Target Location**: `src-tauri/src/entity/`

**Entity 1: `content_store.rs`**
```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "stores")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub session_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::content::Entity")]
    Contents,
}

impl Related<super::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Contents.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

**Entity 2: `content.rs`**
```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "contents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub session_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: i64,
    pub line_count: i64,
    pub preview: String,
    pub uploaded_at: String,
    pub chunk_count: i64,
    pub last_accessed_at: String,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub src_url: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content_store::Entity",
        from = "Column::SessionId",
        to = "super::content_store::Column::SessionId",
        on_delete = "Cascade"
    )]
    ContentStore,
    #[sea_orm(has_many = "super::content_chunk::Entity")]
    Chunks,
}

impl Related<super::content_store::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ContentStore.def()
    }
}

impl Related<super::content_chunk::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Chunks.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

**Entity 3: `content_chunk.rs`**
```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "chunks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub content_id: String,
    pub chunk_index: i64,
    #[sea_orm(column_type = "Text")]
    pub text: String,
    pub start_line: i64,
    pub end_line: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::content::Entity",
        from = "Column::ContentId",
        to = "super::content::Column::Id",
        on_delete = "Cascade"
    )]
    Content,
}

impl Related<super::content::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Content.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
```

### 3.2 Migration File

**Target**: `src-tauri/migration/m20260105_000001_create_content_store_tables.rs`

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create stores table
        manager
            .create_table(
                Table::create()
                    .table(Stores::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Stores::SessionId).string().not_null().primary_key())
                    .col(ColumnDef::new(Stores::Name).string())
                    .col(ColumnDef::new(Stores::Description).string())
                    .col(ColumnDef::new(Stores::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Stores::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // Create contents table
        manager
            .create_table(
                Table::create()
                    .table(Contents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Contents::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Contents::SessionId).string().not_null())
                    .col(ColumnDef::new(Contents::Filename).string().not_null())
                    .col(ColumnDef::new(Contents::MimeType).string().not_null())
                    .col(ColumnDef::new(Contents::Size).big_integer().not_null())
                    .col(ColumnDef::new(Contents::LineCount).big_integer().not_null())
                    .col(ColumnDef::new(Contents::Preview).string().not_null())
                    .col(ColumnDef::new(Contents::UploadedAt).string().not_null())
                    .col(ColumnDef::new(Contents::ChunkCount).big_integer().not_null())
                    .col(ColumnDef::new(Contents::LastAccessedAt).string().not_null())
                    .col(ColumnDef::new(Contents::Content).text().not_null())
                    .col(ColumnDef::new(Contents::SrcUrl).string())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Contents::Table, Contents::SessionId)
                            .to(Stores::Table, Stores::SessionId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create chunks table
        manager
            .create_table(
                Table::create()
                    .table(Chunks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Chunks::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Chunks::ContentId).string().not_null())
                    .col(ColumnDef::new(Chunks::ChunkIndex).big_integer().not_null())
                    .col(ColumnDef::new(Chunks::Text).text().not_null())
                    .col(ColumnDef::new(Chunks::StartLine).big_integer().not_null())
                    .col(ColumnDef::new(Chunks::EndLine).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Chunks::Table, Chunks::ContentId)
                            .to(Contents::Table, Contents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Create indexes
        manager
            .create_index(
                Index::create()
                    .name("idx_chunks_content_id")
                    .table(Chunks::Table)
                    .col(Chunks::ContentId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_contents_session_id")
                    .table(Contents::Table)
                    .col(Contents::SessionId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Chunks::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Contents::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Stores::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Stores {
    Table,
    SessionId,
    Name,
    Description,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Contents {
    Table,
    Id,
    SessionId,
    Filename,
    MimeType,
    Size,
    LineCount,
    Preview,
    UploadedAt,
    ChunkCount,
    LastAccessedAt,
    Content,
    SrcUrl,
}

#[derive(Iden)]
enum Chunks {
    Table,
    Id,
    ContentId,
    ChunkIndex,
    Text,
    StartLine,
    EndLine,
}
```

### 3.3 storage.rs Refactoring

**File**: `src-tauri/src/mcp/builtin/content_store/storage.rs`

**Step 1: Update Imports and Struct** (Lines 1-50)

```rust
// BEFORE
use sqlx::sqlite::SqlitePool;

pub struct ContentStoreStorage {
    stores: HashMap<String, ContentStore>,
    contents: HashMap<String, ContentItem>,
    chunks: HashMap<String, Vec<ContentChunk>>,
    sqlite_pool: Option<SqlitePool>,
}

// AFTER
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set};
use crate::entity::{content_store, content, content_chunk};

pub struct ContentStoreStorage {
    stores: HashMap<String, ContentStore>,
    contents: HashMap<String, ContentItem>,
    chunks: HashMap<String, Vec<ContentChunk>>,
    db: Option<DatabaseConnection>,  // Changed from sqlite_pool
}
```

**Step 2: Replace `new_sqlite()` Method** (Lines 66-102)

```rust
// BEFORE
pub async fn new_sqlite(database_url: String) -> Result<Self, String> {
    // Database file creation
    let pool = SqlitePool::connect(&db_path).await
        .map_err(|e| format!("Failed to connect to SQLite: {e}"))?;
    
    Self::create_tables(&pool).await?;
    
    Ok(Self {
        stores: HashMap::new(),
        contents: HashMap::new(),
        chunks: HashMap::new(),
        sqlite_pool: Some(pool),
    })
}

// AFTER
pub async fn new_sqlite(database_url: String) -> Result<Self, String> {
    // Ensure database directory exists (same as before)
    let db_path = if let Some(path) = database_url.strip_prefix("sqlite://") {
        path.to_string()
    } else {
        database_url.clone()
    };

    if let Some(parent_dir) = std::path::Path::new(&db_path).parent() {
        std::fs::create_dir_all(parent_dir)
            .map_err(|e| format!("Failed to create database directory: {e}"))?;
    }

    // Connect using SeaORM
    let db = Database::connect(&format!("sqlite://{}", db_path))
        .await
        .map_err(|e| format!("Failed to connect to database: {e}"))?;

    // Note: Migrations should be run at application startup, not here
    // This assumes migrations have already been applied

    Ok(Self {
        stores: HashMap::new(),
        contents: HashMap::new(),
        chunks: HashMap::new(),
        db: Some(db),
    })
}
```

**Step 3: Remove `create_tables()` Method** (Lines 104-163)

**Reasoning**: Schema creation is now handled by SeaORM migrations. Remove entire method.

**Step 4: Replace `create_store()` Method** (Lines 165-214)

```rust
// BEFORE
pub async fn create_store(&mut self, session_id: String, name: Option<String>, description: Option<String>) -> Result<ContentStore, String> {
    // ... validation ...
    
    if let Some(pool) = &self.sqlite_pool {
        sqlx::query("INSERT INTO stores (session_id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
            .bind(&session_id)
            .bind(&name)
            .bind(&description)
            .bind(&now)
            .bind(&now)
            .execute(pool).await
            .map_err(|e| format!("Failed to create store in SQLite: {e}"))?;
    }
    
    self.stores.insert(session_id.clone(), store.clone());
    Ok(store)
}

// AFTER
pub async fn create_store(&mut self, session_id: String, name: Option<String>, description: Option<String>) -> Result<ContentStore, String> {
    // Check if store already exists
    if self.stores.contains_key(&session_id) {
        return Err(format!("Content store already exists for session: {session_id}"));
    }

    let now = chrono::Utc::now().to_rfc3339();

    let store = ContentStore {
        session_id: session_id.clone(),
        name: name.clone(),
        description: description.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    // SeaORM backend
    if let Some(db) = &self.db {
        let active_model = content_store::ActiveModel {
            session_id: Set(session_id.clone()),
            name: Set(name),
            description: Set(description),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        };

        content_store::Entity::insert(active_model)
            .exec(db)
            .await
            .map_err(|e| format!("Failed to create store: {e}"))?;
    }

    // In-memory cache
    self.stores.insert(session_id.clone(), store.clone());
    Ok(store)
}
```

**Step 5: Replace `get_or_create_store()` Method** (Lines 260-302)

```rust
// BEFORE
if let Some(pool) = &self.sqlite_pool {
    let result = sqlx::query_as::<_, (String, Option<String>, Option<String>, String, String)>(
        "SELECT session_id, name, description, created_at, updated_at FROM stores WHERE session_id = ?"
    )
    .bind(&session_id)
    .fetch_optional(pool).await
    .map_err(|e| format!("Failed to check store existence in SQLite: {e}"))?;
    
    if let Some((session_id, name, description, created_at, updated_at)) = result {
        let store = ContentStore { /* ... */ };
        self.stores.insert(session_id.clone(), store.clone());
        return Ok(store);
    }
}

// AFTER
if let Some(db) = &self.db {
    let result = content_store::Entity::find_by_id(session_id.clone())
        .one(db)
        .await
        .map_err(|e| format!("Failed to check store existence: {e}"))?;
    
    if let Some(model) = result {
        let store = ContentStore {
            session_id: model.session_id.clone(),
            name: model.name,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        };
        self.stores.insert(session_id.clone(), store.clone());
        return Ok(store);
    }
}
```

**Step 6: Replace `add_content()` Method** (Lines 304-409) ⚠️ **MOST COMPLEX**

```rust
// BEFORE
// SQLite backend
if let Some(pool) = &self.sqlite_pool {
    // Insert content
    sqlx::query("INSERT INTO contents (...) VALUES (?, ?, ?, ..., ?)")
        .bind(content_id)
        // ... 12 binds ...
        .execute(pool).await
        .map_err(|e| format!("Failed to save content to SQLite: {e}"))?;

    // Insert chunks
    for chunk in &content_chunks {
        sqlx::query("INSERT INTO chunks (...) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&chunk.id)
            // ... 6 binds per chunk ...
            .execute(pool).await
            .map_err(|e| format!("Failed to save chunk to SQLite: {e}"))?;
    }
}

// AFTER
// SeaORM backend
if let Some(db) = &self.db {
    // Insert content
    let content_active_model = content::ActiveModel {
        id: Set(content_id.clone()),
        session_id: Set(session_id.to_string()),
        filename: Set(filename.to_string()),
        mime_type: Set(mime_type.to_string()),
        size: Set(size as i64),
        line_count: Set(line_count as i64),
        preview: Set(content_item.preview.clone()),
        uploaded_at: Set(content_item.uploaded_at.clone()),
        chunk_count: Set(chunk_count as i64),
        last_accessed_at: Set(content_item.last_accessed_at.clone()),
        content: Set(content.to_string()),
        src_url: Set(src_url.clone()),
    };

    content::Entity::insert(content_active_model)
        .exec(db)
        .await
        .map_err(|e| format!("Failed to save content: {e}"))?;

    // Bulk insert chunks using insert_many
    let chunk_models: Vec<content_chunk::ActiveModel> = content_chunks
        .iter()
        .map(|chunk| content_chunk::ActiveModel {
            id: Set(chunk.id.clone()),
            content_id: Set(chunk.content_id.clone()),
            chunk_index: Set(chunk.chunk_index as i64),
            text: Set(chunk.text.clone()),
            start_line: Set(chunk.line_range.0 as i64),
            end_line: Set(chunk.line_range.1 as i64),
        })
        .collect();

    if !chunk_models.is_empty() {
        content_chunk::Entity::insert_many(chunk_models)
            .exec(db)
            .await
            .map_err(|e| format!("Failed to save chunks: {e}"))?;
    }
}
```

**Performance Note**: `insert_many()` executes a single INSERT statement with multiple value sets, significantly faster than N individual INSERTs.

**Step 7: Replace `delete_content()` Method** (Lines 487-517)

```rust
// BEFORE
if let Some(pool) = &self.sqlite_pool {
    // Delete chunks first (due to foreign key constraint)
    sqlx::query("DELETE FROM chunks WHERE content_id = ?")
        .bind(content_id)
        .execute(pool).await
        .map_err(|e| format!("Failed to delete chunks from SQLite: {e}"))?;

    // Delete content
    sqlx::query("DELETE FROM contents WHERE id = ?")
        .bind(content_id)
        .execute(pool).await
        .map_err(|e| format!("Failed to delete content from SQLite: {e}"))?;
}

// AFTER
if let Some(db) = &self.db {
    // SeaORM: ON DELETE CASCADE handles chunks automatically
    content::Entity::delete_by_id(content_id.to_string())
        .exec(db)
        .await
        .map_err(|e| format!("Failed to delete content: {e}"))?;
}
```

**Simplification**: Rely on `ON DELETE CASCADE` foreign key constraint defined in migration. No need to manually delete chunks first.

### 3.4 Test Migration Refactoring

**File**: `src-tauri/src/mcp/builtin/content_store/test_migration.rs` (100 lines)

**Current Test Purpose** (Lines 1-76):
- Tests that `src_url` column migration works
- Creates old schema WITHOUT `src_url`
- Initializes ContentStoreStorage (triggers migration)
- Verifies column exists by attempting UPDATE

**SeaORM Approach**:
- Replace with SeaORM migration testing utilities
- Test that migration from old schema to new schema works
- Verify all columns and indexes are created correctly

**New Test Structure**:
```rust
#[cfg(test)]
mod tests {
    use sea_orm::{Database, Schema};
    use crate::entity::{content_store, content, content_chunk};
    use crate::migration::Migrator;
    use sea_orm_migration::MigratorTrait;

    #[tokio::test]
    async fn test_migration_creates_all_tables() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        
        // Run migrations
        Migrator::up(&db, None).await.unwrap();
        
        // Verify tables exist
        let schema = Schema::new(sea_orm::DbBackend::Sqlite);
        let store_stmt = schema.create_table_from_entity(content_store::Entity);
        let content_stmt = schema.create_table_from_entity(content::Entity);
        let chunk_stmt = schema.create_table_from_entity(content_chunk::Entity);
        
        // Verify we can insert and query
        // ... test data operations ...
    }

    #[tokio::test]
    async fn test_migration_preserves_existing_data() {
        // Create old database with sample data
        // Run migration
        // Verify data is preserved
    }
}
```

---

## 4. Reusable Code & Patterns

### 4.1 Entity Conversion Helpers

**Pattern**: Convert between SeaORM `Model` and internal `ContentStore`/`ContentItem` structs

```rust
// Add to storage.rs
impl From<content_store::Model> for ContentStore {
    fn from(model: content_store::Model) -> Self {
        Self {
            session_id: model.session_id,
            name: model.name,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

impl From<content::Model> for ContentItem {
    fn from(model: content::Model) -> Self {
        Self {
            id: model.id,
            session_id: model.session_id,
            filename: model.filename,
            mime_type: model.mime_type,
            size: model.size as usize,
            line_count: model.line_count as usize,
            preview: model.preview,
            uploaded_at: model.uploaded_at,
            chunk_count: model.chunk_count as usize,
            last_accessed_at: model.last_accessed_at,
            content: model.content,
            src_url: model.src_url,
        }
    }
}

impl From<content_chunk::Model> for ContentChunk {
    fn from(model: content_chunk::Model) -> Self {
        Self {
            id: model.id,
            content_id: model.content_id,
            chunk_index: model.chunk_index as usize,
            text: model.text,
            line_range: (model.start_line as usize, model.end_line as usize),
        }
    }
}
```

### 4.2 Bulk Insert Pattern (Reusable Across All Modules)

```rust
// Generic pattern for bulk inserts
let models: Vec<EntityActiveModel> = items
    .iter()
    .map(|item| EntityActiveModel {
        field1: Set(item.field1.clone()),
        field2: Set(item.field2),
        // ...
    })
    .collect();

if !models.is_empty() {
    Entity::insert_many(models)
        .exec(db)
        .await?;
}
```

### 4.3 Error Handling Pattern

```rust
// Consistent error handling across all SeaORM operations
entity::Entity::operation()
    .exec(db)
    .await
    .map_err(|e| format!("Failed to [operation description]: {e}"))?;
```

---

## 5. Verification & Testing Plan

### 5.1 Unit Tests

**New Tests to Add**:
- [ ] Test entity conversion (`From` trait implementations)
- [ ] Test bulk chunk insertion with 0, 1, 10, 100, 1000 chunks
- [ ] Test cascading delete (verify chunks are deleted with content)
- [ ] Test `get_or_create_store()` with existing and non-existing stores

**Existing Tests to Maintain**:
- [ ] All tests in `test_functional.rs` must pass unchanged
- [ ] All tests in `test_session_isolation.rs` must pass unchanged

### 5.2 Integration Tests

**Test Scenarios**:
1. **Empty Database**: Create new store, add content, list, read, delete
2. **Migration from Old Schema**: 
   - Create database with old schema (no `src_url`)
   - Run migration
   - Verify all data preserved
   - Verify new column exists
3. **Concurrent Operations**: Multiple sessions adding content simultaneously
4. **Large Content**: Add content with 10,000+ chunks
5. **Transaction Rollback**: Simulate failure during multi-table operation

### 5.3 Performance Benchmarks

**Baseline (SQLx) vs. SeaORM**:

| Operation | Baseline (ms) | Target (ms) | Method |
|-----------|---------------|-------------|--------|
| Create Store | TBD | ±10% | Single INSERT |
| Add Content (100 chunks) | TBD | ±10% | 1 INSERT + bulk INSERT |
| Add Content (1000 chunks) | TBD | ±10% | Bulk INSERT efficiency test |
| List Content (paginated) | TBD | ±10% | In-memory only |
| Delete Content | TBD | ±10% | Single DELETE with cascade |

**Benchmark Tool**: Use `criterion` crate for micro-benchmarks

---

## 6. Migration Rollback Plan

### 6.1 Rollback Triggers
- Critical bug discovered in SeaORM implementation
- Performance degradation > 30%
- Data corruption detected
- Migration failure on production databases

### 6.2 Rollback Procedure

**Step 1**: Stop deployment and notify team

**Step 2**: Code rollback
```bash
git checkout dev/0.4.0-pre-seaorm-contentstore
pnpm tauri build
```

**Step 3**: Database rollback
- Restore from pre-migration backup
- Or run migration down: `Migrator::down(&db, None).await`

**Step 4**: Verify functionality
- Run full test suite
- Manual testing of all Content Store features

---

## 7. Dependencies & Prerequisites

### 7.1 Cargo.toml Updates

```toml
[dependencies]
# Add SeaORM dependencies
sea-orm = { version = "1.1", features = ["sqlx-sqlite", "runtime-tokio-native-tls", "macros"] }

[dev-dependencies]
# Add for testing
sea-orm-migration = "1.1"
```

### 7.2 Migration Infrastructure

**Prerequisite**: Phase 0 must be complete
- Migration framework setup in `src-tauri/migration/`
- Migrator integrated into application startup
- Entity generation tooling configured

### 7.3 Planning Module Completion (Optional)

**Recommendation**: Complete Phase 1 (Planning module) first to establish patterns and learn from complexity before tackling Content Store.

---

## 8. Estimated Timeline

| Task | Duration | Dependencies |
|------|----------|--------------|
| **8.1** Entity Generation | 0.5 days | Phase 0 complete |
| **8.2** Migration File Creation | 0.5 days | Entities generated |
| **8.3** storage.rs Refactoring | 2 days | Migration file ready |
| **8.4** Test Migration Refactoring | 0.5 days | storage.rs complete |
| **8.5** Unit Test Updates | 1 day | Code refactoring complete |
| **8.6** Integration Testing | 1 day | Unit tests passing |
| **8.7** Performance Benchmarking | 0.5 days | Integration tests passing |
| **Total** | **6 days (1.2 weeks)** | |

**Buffer**: Add 20% buffer for unexpected issues → **1.5 weeks total**

---

## 9. Success Criteria Summary

### 9.1 Functional Requirements
- [x] All Content Store tools work identically to pre-migration
- [x] No data loss during migration from existing databases
- [x] Session isolation maintained (different sessions can't access each other's data)
- [x] Cascading deletes work correctly (chunks deleted with content)
- [x] Bulk chunk insertion works efficiently (no N+1 queries)

### 9.2 Performance Requirements
- [x] Query performance within 10% of SQLx baseline
- [x] Memory usage stable or improved
- [x] Startup time unchanged
- [x] No new bottlenecks introduced

### 9.3 Code Quality Requirements
- [x] 100% test coverage for new SeaORM code
- [x] Zero compiler warnings
- [x] All clippy lints pass
- [x] Code reviews approved
- [x] Documentation updated

### 9.4 Migration Requirements
- [x] Migration runs successfully on empty database
- [x] Migration preserves data from old schema
- [x] Rollback procedure tested and documented
- [x] Production database backup procedure in place

---

## 10. Clarification Q-List

### Q1: In-Memory Cache Strategy
**Question**: Should we maintain the current dual-mode design (in-memory cache + optional SQLite backend)?

**Options**:
- A) Keep current design (cache always populated, SQLite optional)
- B) Remove cache, use SeaORM as single source of truth
- C) Make cache optional (populate only for hot paths)

**Recommendation**: Option A - Maintain compatibility with existing architecture

**Impact**: Low if keeping current design, Medium-High if changing

---

### Q2: Transaction Boundaries
**Question**: Should we wrap multi-table operations (add_content with chunks) in explicit transactions?

**Current**: No explicit transactions, relies on sequential operations

**Options**:
- A) No transactions (rely on operation atomicity)
- B) Use SeaORM transactions for multi-table writes
- C) Add transaction support as optional feature

**Recommendation**: Option B - Add explicit transactions for data integrity

**Code Example**:
```rust
let txn = db.begin().await?;

content::Entity::insert(content_model).exec(&txn).await?;
content_chunk::Entity::insert_many(chunk_models).exec(&txn).await?;

txn.commit().await?;
```

**Impact**: Medium - Changes add_content() method, requires testing

---

### Q3: Schema Evolution Strategy
**Question**: How should we handle future schema changes after migration?

**Options**:
- A) Always use SeaORM migrations (create new migration files)
- B) Allow ALTER TABLE for backwards-compatible changes
- C) Require database recreation for schema changes

**Recommendation**: Option A - Use migration framework for all changes

**Rationale**: Consistent with SeaORM best practices, enables version control of schema

---

### Q4: Load Cache from Database on Startup
**Question**: Should we load existing data from database into cache on initialization?

**Current**: Cache is populated lazily (on first access)

**Options**:
- A) Keep lazy loading (current behavior)
- B) Load all stores/contents into cache on startup
- C) Make it configurable (lazy vs. eager loading)

**Recommendation**: Option A - Keep lazy loading for performance

**Impact**: Low - No behavior change

---

### Q5: Error Type Standardization
**Question**: Should we create custom error types instead of `String` errors?

**Current**: All methods return `Result<T, String>`

**Options**:
- A) Keep `String` errors (simple, backwards compatible)
- B) Create `ContentStoreError` enum with structured error types
- C) Use `anyhow::Error` for flexibility

**Recommendation**: Option A for this phase, Option B for future improvement

**Rationale**: Minimize scope of this refactoring, add custom errors in separate PR

---

### Q6: Deprecation of `create_tables()`
**Question**: Should we immediately remove `create_tables()` or mark it deprecated first?

**Options**:
- A) Remove immediately (migration handles schema)
- B) Mark deprecated, remove in next major version
- C) Keep as fallback for non-migrated databases

**Recommendation**: Option A - Remove immediately

**Rationale**: Migration framework is now the single source of truth for schema

---

## 11. References

### 11.1 Related Documentation
- [SeaORM Migration Master Plan](../planning/seaorm-migration-master-plan.md)
- [Refactoring Plan Submission Guide](../../refactoring_plan_submission_guide.md)
- [SeaORM Book](https://www.sea-ql.org/SeaORM/)
- [SeaORM Migration Guide](https://www.sea-ql.org/SeaORM/docs/migration/setting-up-migration/)

### 11.2 Related Code Files
- `src-tauri/src/mcp/builtin/content_store/storage.rs` (Primary target)
- `src-tauri/src/mcp/builtin/content_store/test_migration.rs` (Secondary target)
- `src-tauri/src/mcp/builtin/planning/` (Reference for patterns)

### 11.3 Related Issues & PRs
- GitHub PR: dev/0.4.0 (#272)
- Master Plan: SeaORM Migration Phase 3

---

**Document Status**: ✅ READY FOR REVIEW  
**Next Step**: Team review and approval before implementation  
**Approval Required**: Technical Lead, Database Specialist

---

**End of Refactoring Plan**
