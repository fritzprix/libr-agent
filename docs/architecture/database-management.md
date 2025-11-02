# Database Management Architecture

## Overview

LibrAgent uses a dual-storage architecture combining **SQLite** for persistent data storage and **in-memory caching** for performance optimization. The database layer is managed through a **Repository Pattern** implementation that provides abstraction, testability, and clean separation of concerns.

## Architecture Components

### 1. Database Stack

```text
┌─────────────────────────────────────┐
│   Tauri Commands Layer             │
│   (Frontend-Backend Interface)      │
└─────────────────────────────────────┘
            ↓
┌─────────────────────────────────────┐
│   Repository Trait Layer            │
│   (Abstraction & Interface)         │
└─────────────────────────────────────┘
            ↓
┌─────────────────────────────────────┐
│   SQLite Implementation Layer       │
│   (Concrete Repository Classes)     │
└─────────────────────────────────────┘
            ↓
┌─────────────────────────────────────┐
│   SQLx Connection Pool              │
│   (Database Connection Management)  │
└─────────────────────────────────────┘
            ↓
┌─────────────────────────────────────┐
│   SQLite Database File              │
│   (Persistent Storage)              │
└─────────────────────────────────────┘
```

### 2. Repository Pattern

The project implements the **Repository Pattern** to abstract database operations:

**Location**: `src-tauri/src/repositories/`

```text
repositories/
├── mod.rs                          # Module exports and re-exports
├── error.rs                        # Database error types
├── message_repository.rs           # Message CRUD operations
├── session_repository.rs           # Session metadata operations
└── content_store_repository.rs     # Content store data operations
```

## Core Components

### 2.1 Error Handling (`error.rs`)

Centralized error handling for all database operations:

```rust
#[derive(Debug, Error)]
pub enum DbError {
    /// Database query execution failed
    #[error("Database query failed: {0}")]
    QueryFailed(#[from] sqlx::Error),

    /// Requested record was not found
    #[error("Record not found: {0}")]
    NotFound(String),

    /// Invalid input parameter provided
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Transaction commit or rollback failed
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
}
```

**Features**:

- Implements `thiserror::Error` for structured error handling
- Automatic conversion from `sqlx::Error`
- Converts to `String` for Tauri command compatibility
- Provides context-specific error messages

### 2.2 Message Repository (`message_repository.rs`)

Manages chat messages, pagination, and search index metadata.

#### Trait Definition

```rust
#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create_table(&self) -> Result<(), DbError>;
    async fn get_page(&self, session_id: &str, page: usize, page_size: usize)
        -> Result<Page<Message>, DbError>;
    async fn insert(&self, message: &Message) -> Result<(), DbError>;
    async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError>;
    async fn delete_by_id(&self, message_id: &str) -> Result<(), DbError>;
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError>;
    async fn update_index_meta(&self, session_id: &str, index_path: &str,
        doc_count: usize, rebuild_duration_ms: i64) -> Result<(), DbError>;
    async fn get_last_indexed_at(&self, session_id: &str) -> Result<i64, DbError>;
    async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError>;
}
```

#### Messages Schema

**`messages` Table**:

```sql
CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL CHECK(session_id <> ''),
    role TEXT NOT NULL,                    -- 'user', 'assistant', 'system', 'tool'
    content TEXT NOT NULL,
    tool_calls TEXT,                       -- JSON serialized tool calls
    tool_call_id TEXT,                     -- Reference to tool call
    is_streaming INTEGER,                  -- Boolean flag
    thinking TEXT,                         -- Reasoning/thinking content
    thinking_signature TEXT,               -- Signature for thinking
    assistant_id TEXT,                     -- ID of assistant that generated message
    attachments TEXT,                      -- JSON serialized attachments
    tool_use TEXT,                         -- JSON serialized tool usage
    created_at INTEGER NOT NULL,           -- Unix timestamp in milliseconds
    updated_at INTEGER NOT NULL,           -- Unix timestamp in milliseconds
    source TEXT,                           -- Message source identifier
    error TEXT                             -- Error message if any
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_messages_session_created
    ON messages(session_id, created_at);

CREATE INDEX IF NOT EXISTS idx_messages_session_id
    ON messages(session_id);
```

**`message_index_meta` Table**:

```sql
CREATE TABLE IF NOT EXISTS message_index_meta (
    session_id TEXT PRIMARY KEY,
    index_path TEXT,                       -- Path to BM25 index file
    last_indexed_at INTEGER DEFAULT 0,     -- Last index build timestamp
    doc_count INTEGER DEFAULT 0,           -- Number of documents in index
    index_version INTEGER DEFAULT 1,       -- Index format version
    last_rebuild_duration_ms INTEGER       -- Time taken for last rebuild
);
```

#### Key Operations

**1. Pagination with Performance**:

```rust
async fn get_page(&self, session_id: &str, page: usize, page_size: usize)
    -> Result<Page<Message>, DbError>
```

- Validates page_size > 0
- Calculates offset for pagination
- Returns total count, has_next/has_prev flags
- Ordered by `created_at ASC`

**2. Upsert Pattern**:

```rust
async fn insert(&self, message: &Message) -> Result<(), DbError>
```

- Uses `ON CONFLICT(id) DO UPDATE` for idempotent operations
- All fields updated except `created_at`
- Supports message updates during streaming

**3. Bulk Insert with Transactions**:

```rust
async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError>
```

- Wraps operations in a transaction
- Atomic batch inserts
- Rolls back on any error

**4. Search Index Tracking**:

```rust
async fn is_index_dirty(&self, session_id: &str) -> Result<bool, DbError>
```

- Compares `max(created_at)` with `last_indexed_at`
- Determines if index rebuild is needed
- Used by background indexing worker

### 2.3 Content Store Repository (`content_store_repository.rs`)

Manages content store data cleanup for sessions.

#### Content Store Trait

```rust
#[async_trait]
pub trait ContentStoreRepository: Send + Sync {
    async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError>;
}
```

#### Content Store Schema

**`stores` Table**:

```sql
CREATE TABLE IF NOT EXISTS stores (
    session_id TEXT PRIMARY KEY,          -- 1:1 relationship with session
    name TEXT,
    description TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

**`contents` Table**:

```sql
CREATE TABLE IF NOT EXISTS contents (
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
    content TEXT NOT NULL,                 -- Full file content
    FOREIGN KEY (session_id) REFERENCES stores(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_contents_session_id ON contents(session_id);
```

**`chunks` Table**:

```sql
CREATE TABLE IF NOT EXISTS chunks (
    id TEXT PRIMARY KEY,
    content_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    text TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    FOREIGN KEY (content_id) REFERENCES contents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chunks_content_id ON chunks(content_id);
```

#### Content Store Operations

**Cascade Deletion**:

```rust
async fn delete_by_session(&self, session_id: &str) -> Result<(), DbError>
```

1. Delete `chunks` first (respects foreign key constraints)
2. Delete `contents`
3. Delete `stores`

This ensures proper cleanup when a session is deleted.

### 2.4 Session Repository (`session_repository.rs`)

Manages session-level metadata operations.

#### Trait Definition

```rust
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError>;
}
```

#### Session Operations

**Index Metadata Cleanup**:

```rust
async fn delete_index_metadata(&self, session_id: &str) -> Result<(), DbError>
```

- Removes search index metadata when session is deleted
- Coordinates with file system index deletion
- Part of complete session cleanup workflow

## Database Initialization

### Location: `src-tauri/src/lib.rs`

The database initialization follows a strict sequence:

```rust
pub fn run_with_sqlite_sync(db_url: String) {
    let rt = tokio::runtime::Runtime::new()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async {
        // 1. Initialize SQLite connection pool
        let pool = sqlx::sqlite::SqlitePool::connect(&db_url)
            .await
            .expect("Failed to connect to SQLite");

        // 2. Create repository instances
        let message_repo = SqliteMessageRepository::new(pool.clone());
        let content_store_repo = SqliteContentStoreRepository::new(pool.clone());
        let session_repo = SqliteSessionRepository::new(pool.clone());

        // 3. Initialize tables
        message_repo.create_table().await
            .expect("Failed to create messages table");

        // 4. Set global instances
        set_sqlite_pool(pool);
        set_message_repository(message_repo);
        set_content_store_repository(content_store_repo);
        set_session_repository(session_repo);

        // 5. Initialize background workers
        let _indexing_worker = search::IndexingWorker::new(
            std::time::Duration::from_secs(300)
        );
    });

    run();
}
```

### Database File Creation

If the database file doesn't exist:

1. Extract path from `sqlite://` URL
2. Create parent directories with `std::fs::create_dir_all()`
3. Create empty database file with `std::fs::File::create()`
4. Retry connection
5. Run schema initialization

This ensures zero-configuration startup for new installations.

## Connection Pooling

### SQLx Connection Pool

**Configuration**:

- Pool is shared across all repositories via `SqlitePool::clone()`
- Cloning the pool is cheap (Arc-based reference counting)
- Connections are automatically managed by SQLx
- Pool is stored in global state for access from Tauri commands

**Benefits**:

- Efficient connection reuse
- Automatic connection lifecycle management
- Thread-safe access across async tasks
- Prevents connection exhaustion

## Transaction Management

### Single Transaction Pattern

Used for bulk operations requiring atomicity:

```rust
async fn insert_many(&self, messages: Vec<Message>) -> Result<(), DbError> {
    let mut tx = self.pool.begin().await?;

    for message in messages {
        sqlx::query("INSERT INTO messages ...")
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await
        .map_err(|e| DbError::TransactionFailed(e.to_string()))?;
    Ok(())
}
```

**Key Points**:

- Explicit transaction boundaries
- Automatic rollback on error (via Drop)
- Custom error conversion for `TransactionFailed`
- Ensures data consistency

## Background Indexing System

### IndexingWorker

**Location**: Started in `run_with_sqlite_sync()`

```rust
let _indexing_worker = search::IndexingWorker::new(
    std::time::Duration::from_secs(300)  // Check every 5 minutes
);
```

**Workflow**:

1. Periodically checks `is_index_dirty()` for each session
2. Rebuilds BM25 search index if new messages exist
3. Updates `message_index_meta` table with rebuild timestamp
4. Persists index to disk for fast startup

**Benefits**:

- Automatic search index maintenance
- No manual intervention required
- Efficient incremental updates
- Background processing doesn't block UI

## Hybrid Storage Strategy

### Content Store Dual-Backend

The content store uses a **hybrid approach**:

```rust
pub struct ContentStoreStorage {
    // In-memory cache for fast access
    stores: HashMap<String, ContentStore>,
    contents: HashMap<String, ContentItem>,
    chunks: HashMap<String, Vec<ContentChunk>>,

    // SQLite for persistence
    sqlite_pool: Option<SqlitePool>,
}
```

**Strategy**:

1. **SQLite Backend**: Write-through to database
2. **In-Memory Cache**: Read from HashMap for performance
3. **Lazy Initialization**: Cache populated on first access
4. **Session-Scoped**: Each session has isolated content store

**Advantages**:

- Fast read access (HashMap lookup)
- Persistent storage across restarts
- Flexibility to run without SQLite (testing/development)
- Clear separation of concerns

## Data Flow Examples

### 1. Message Creation Flow

```text
User sends message
       ↓
Frontend calls Tauri command
       ↓
Command creates Message struct
       ↓
message_repository.insert(message)
       ↓
UPSERT into messages table
       ↓
IndexingWorker marks session as dirty
       ↓
Background rebuild of BM25 index
       ↓
Update message_index_meta
```

### 2. Session Deletion Flow

```text
User deletes session
       ↓
session_commands::delete_session()
       ↓
Delete BM25 index file and metadata
       ↓
session_repository.delete_index_metadata()
       ↓
content_store_repository.delete_by_session()
  ├─ DELETE chunks
  ├─ DELETE contents
  └─ DELETE stores
       ↓
message_repository.delete_by_session()
       ↓
Delete session workspace directory
```

### 3. Content Store Upload Flow

```text
User uploads file via MCP tool
       ↓
ContentStoreServer receives request
       ↓
Parse file content (PDF/Excel/Markdown/etc)
       ↓
Create ContentItem with full content
       ↓
Chunk content (configurable chunk_size)
       ↓
ContentStoreStorage.add_content()
  ├─ INSERT into contents table
  └─ INSERT chunks into chunks table
       ↓
Update in-memory cache
       ↓
Return success response
```

## Migration Strategy

### Current Approach: Schema Initialization

The project currently uses **CREATE TABLE IF NOT EXISTS** pattern:

```rust
async fn create_table(&self) -> Result<(), DbError> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS messages (...);
        CREATE INDEX IF NOT EXISTS idx_messages_session_created ...;
    "#)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

### Advantages

- Simple and reliable for new installations
- No migration tracking overhead
- Idempotent schema creation

### Limitations

- Cannot modify existing schemas
- No version tracking
- Manual intervention required for schema changes

### Future Recommendations

For production evolution, consider:

1. **SQLx Migrations**:

   ```bash
   sqlx migrate add initial_schema
   sqlx migrate run
   ```

2. **Version Tracking**:

   ```sql
   CREATE TABLE schema_version (
       version INTEGER PRIMARY KEY,
       applied_at TEXT NOT NULL
   );
   ```

3. **Incremental Migrations**:
   - Each schema change gets a migration file
   - Migrations run automatically on startup
   - Rollback support for failed migrations

## Performance Optimization

### 1. Indexing Strategy

**Messages Table**:

- `idx_messages_session_created`: Composite index for pagination queries
- `idx_messages_session_id`: Fast session-based filtering

**Contents/Chunks Tables**:

- `idx_contents_session_id`: Session-level content queries
- `idx_chunks_content_id`: Chunk retrieval by content ID

### 2. Query Optimization

**Pagination**:

```sql
-- Efficient: Uses index for ORDER BY and WHERE
SELECT * FROM messages
WHERE session_id = ?
ORDER BY created_at ASC
LIMIT ? OFFSET ?
```

**Count Query**:

```sql
-- Optimized: COUNT(1) instead of COUNT(*)
SELECT COUNT(1) as count FROM messages WHERE session_id = ?
```

### 3. Upsert Pattern

```sql
INSERT INTO messages (...) VALUES (...)
ON CONFLICT(id) DO UPDATE SET ...
```

**Benefits**:

- Single query for insert/update
- Idempotent operations
- Reduces race conditions
- Supports streaming message updates

## Error Handling Best Practices

### 1. Structured Error Types

```rust
match message_repo.get_page(session_id, page, page_size).await {
    Ok(page) => Ok(page),
    Err(DbError::InvalidInput(msg)) => {
        // Handle validation errors
        Err(format!("Invalid input: {msg}"))
    }
    Err(DbError::NotFound(msg)) => {
        // Handle missing data
        Err(format!("Not found: {msg}"))
    }
    Err(e) => {
        // Handle other database errors
        Err(format!("Database error: {e}"))
    }
}
```

### 2. Custom Transaction Error Handling

```rust
tx.commit().await
    .map_err(|e| DbError::TransactionFailed(e.to_string()))?;
```

- Custom error conversion
- Preserves error context
- Automatic rollback on panic or early return

### 3. Tauri Command Compatibility

```rust
impl From<DbError> for String {
    fn from(err: DbError) -> String {
        err.to_string()
    }
}
```

- Seamless conversion for Tauri commands
- Error messages propagate to frontend
- Structured logging in backend

## Testing Strategies

### 1. Repository Testing

**Mock Implementation**:

```rust
struct MockMessageRepository {
    messages: Arc<Mutex<HashMap<String, Vec<Message>>>>,
}

#[async_trait]
impl MessageRepository for MockMessageRepository {
    async fn insert(&self, message: &Message) -> Result<(), DbError> {
        // In-memory implementation for testing
    }
}
```

### 2. Integration Testing

```rust
#[tokio::test]
async fn test_message_pagination() {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    let repo = SqliteMessageRepository::new(pool);
    repo.create_table().await.unwrap();

    // Test pagination logic
}
```

### 3. Transaction Testing

```rust
#[tokio::test]
async fn test_transaction_rollback() {
    let repo = SqliteMessageRepository::new(pool);

    // Trigger error during insert_many
    let result = repo.insert_many(invalid_messages).await;
    assert!(result.is_err());

    // Verify no partial inserts
    let count = get_message_count().await;
    assert_eq!(count, 0);
}
```

## Global State Management

### Repository Access Pattern

```rust
// Global state storage (in lib.rs or state.rs)
static MESSAGE_REPOSITORY: OnceCell<Arc<dyn MessageRepository>> = OnceCell::new();

pub fn set_message_repository(repo: impl MessageRepository + 'static) {
    MESSAGE_REPOSITORY.set(Arc::new(repo))
        .expect("MESSAGE_REPOSITORY already set");
}

pub fn get_message_repository() -> Arc<dyn MessageRepository> {
    MESSAGE_REPOSITORY.get()
        .expect("MESSAGE_REPOSITORY not initialized")
        .clone()
}
```

**Benefits**:

- Singleton pattern for repository instances
- Thread-safe access via `Arc`
- Lazy initialization during startup
- No need to pass repositories through function parameters

## Related Documentation

- [Chat Feature Architecture](./chat-feature-architecture.md) - How messages integrate with chat UI
- [Content Store MCP Integration](../mcp/content-store.md) - Content store MCP server implementation
- [Search & Indexing](../features/search-indexing.md) - BM25 search implementation
- [Tauri Commands API](../api/tauri-commands.md) - Database command exposures

## Summary

The database management system in LibrAgent provides:

1. ✅ **Clean Architecture**: Repository pattern with trait-based abstraction
2. ✅ **Type Safety**: Rust's type system prevents runtime database errors
3. ✅ **Performance**: Connection pooling, indexing, and caching strategies
4. ✅ **Maintainability**: Clear separation of concerns and testable design
5. ✅ **Reliability**: Transaction support and error handling
6. ✅ **Scalability**: Background workers and hybrid storage approach
7. ✅ **Zero Configuration**: Automatic database and schema initialization

This architecture supports the project's goal of providing a robust, desktop-first AI agent platform while maintaining code quality and developer experience.
