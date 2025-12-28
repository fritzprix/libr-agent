# Refactoring Plan: Built-in MCP Servers Completion

**Date**: 2024-12-27 15:00  
**Target Branch**: dev/0.4.0  
**Scope**: Complete implementation of remaining built-in MCP servers  
**Excluded**: Agent V2 Backend implementation (separate task), Frontend integration

---

## 1. 작업의 목적

Agent V2 Architecture의 Built-in MCP Servers 중 미완성 서버 3개를 완성하여 전체 MCP ecosystem을 production-ready 상태로 만듭니다.

**주요 목표:**

- ✅ AssistantServer: CRUD operations 완성 (현재 30% → 100%)
- ✅ ContentStoreServer: Session-scoped 리팩토링 (현재 50% → 100%)
- ✅ WorkspaceServer: BuiltinMCPServer trait 통합 검증 및 개선 (현재 80% → 100%)

**비즈니스 가치:**

- AssistantServer: Agent configuration 동적 관리 가능
- ContentStoreServer: Session별 독립적인 content 저장소 제공
- WorkspaceServer: File/terminal operations의 session isolation 보장

---

## 2. 현재 상태 / 문제점

### 2.1. AssistantServer (30% 완성)

**파일**: `src-tauri/src/mcp/builtin/assistant/mod.rs`

**현재 상태:**

```rust
✅ Database schema (assistants table)
✅ BuiltinMCPServer trait implementation
✅ Tool definitions (5 tools)
✅ call_tool() routing logic
⚠️ CRUD operations: 기본 구조만 존재, 구현 미완성
```

**문제점:**

1. **create_assistant()**: Skeleton만 존재 (lines 67-108)
   - INSERT logic 있지만 에러 처리 부족
   - Duplicate key handling 없음
2. **update_assistant()**: 미구현 (선언만 존재)
   - UPDATE logic 필요
   - Partial update 지원 필요

3. **delete_assistant()**: 미구현
   - Cascade deletion 전략 미정의
   - Foreign key 관계 고려 필요

4. **list_assistants()**: 기본 구현 있음
   - Pagination 없음
   - Filtering 옵션 없음

5. **get_assistant()**: 기본 구현 있음
   - 에러 메시지 개선 필요

**테스트 상태:**

- Unit tests: 0%
- Integration tests: 0%

### 2.2. ContentStoreServer (50% 완성)

**파일**: `src-tauri/src/mcp/builtin/content_store/server.rs`

**현재 상태:**

```rust
✅ Basic server structure
✅ Storage backend (ContentStoreStorage with SQLite/in-memory dual support)
✅ Search engine (BM25)
✅ Tool handlers (addContent, listContent, readContent, keywordSimilaritySearch, deleteContent)
✅ SQLite persistent storage (Rust Built-in 초기 구현부터 포함)
⚠️ Session isolation: Storage has session_id FK but not used in queries
⚠️ Constructor: Uses in-memory by default, SQLite option unused
```

**문제점:**

1. **Session Isolation 부분 구현됨 but 미활용**:

   ```rust
   // storage.rs:8-20 - Schema는 session_id FK 지원 ✅
   pub struct ContentStore {
       pub session_id: String, // ✅ Primary key
   }
   pub struct ContentItem {
       pub session_id: String, // ✅ FK to ContentStore
   }

   // server.rs:21-30 - BUT constructor가 session_id 활용 안 함 ❌
   pub fn new(session_manager: Arc<SessionManager>) -> Self {
       Self {
           session_manager,
           storage: Mutex::new(storage::ContentStoreStorage::new()),
           // ❌ new() 호출 시 session_id 전달 안 함
       }
   }
   ```

2. **Storage 초기화 문제**:
   - `new()` 사용 → in-memory (session isolation 미작동)
   - `new_with_sqlite()` 있지만 사용 안 됨 → persistent storage 미작동
   - SQLite 구현은 있지만 활용 안 됨 (constructor가 in-memory 기본값)

3. **Query 필터링 누락**:
   - Storage에 session_id FK 있지만 queries가 WHERE session_id = ? 없음
   - `list()`, `get()` 등의 메서드가 global scope로 동작

**영향:**

- Session A의 content가 Session B에서 보일 수 있음
- SQLite persistent storage 기능이 있지만 사용 안 됨
- Content ID 충돌 가능성

**테스트 상태:**

- test_functional.rs: ✅ 기본 기능 테스트 있음
- test_migration.rs: ✅ Migration 테스트 있음
- Session isolation test: ❌ 없음

### 2.3. WorkspaceServer (80% 완성)

**파일**: `src-tauri/src/mcp/builtin/workspace/mod.rs`

**현재 상태:**

```rust
✅ BuiltinMCPServer trait implementation (완전)
✅ Session-based workspace management
✅ File operations (file_operations.rs)
✅ Terminal operations (terminal_manager.rs)
✅ Code execution (code_execution.rs)
✅ Persistent shell (persistent_shell_manager.rs)
✅ Export operations (export_operations.rs)
⚠️ Integration verification needed
```

**문제점:**

1. **Trait Integration 검증 필요**:
   - BuiltinMCPServer trait은 구현되어 있음
   - MCPServiceProxyManager와의 통합 테스트 필요
   - Session isolation 동작 검증 필요

2. **Context Switching**:

   ```rust
   // lines 755-770
   async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
       // ✅ 구현되어 있지만 실제 동작 검증 필요
   }
   ```

3. **Process Cleanup**:
   - Background cleanup task 있음 (24-hour retention)
   - Session 종료 시 즉시 cleanup 로직 확인 필요

**테스트 상태:**

- Unit tests: 60% (file operations, terminal)
- Integration tests: 30% (proxy manager 통합)
- E2E tests: 0%

---

## 3. 관련 코드의 구조 및 동작 방식 Summary (Bird's Eye View)

### 3.1. Built-in Server Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  BuiltinMCPServer Trait                     │
│  - name() / description()                                    │
│  - tools() / call_tool()                                     │
│  - get_service_context() / switch_context()                 │
└───────────────────────┬─────────────────────────────────────┘
                        │ implements
        ┌───────────────┼───────────────┬────────────────┐
        ↓               ↓               ↓                ↓
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  Bootstrap   │ │  Knowledge   │ │   Planning   │ │   Playbook   │
│   Server     │ │    Server    │ │    Server    │ │    Server    │
│  (Stateless) │ │ (Session-    │ │ (Session-    │ │ (Session-    │
│   ✅ 100%    │ │   Scoped)    │ │   Scoped)    │ │   Scoped)    │
│              │ │   ✅ 100%    │ │   ✅ 100%    │ │   ✅ 100%    │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘

        ┌───────────────┬───────────────┬────────────────┐
        ↓               ↓               ↓                ↓
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│  Assistant   │ │ ContentStore │ │  Workspace   │
│   Server     │ │    Server    │ │    Server    │
│   (Global)   │ │ (Session-    │ │ (Session-    │
│   ⚠️ 30%     │ │   Scoped)    │ │   Scoped)    │
│              │ │   ⚠️ 50%     │ │   ⚠️ 80%     │
└──────────────┘ └──────────────┘ └──────────────┘
```

### 3.2. Session Isolation Pattern (Reference: KnowledgeServer)

```rust
// ✅ Correct Pattern (from knowledge/mod.rs)
pub struct KnowledgeServer {
    db_pool: Arc<SqlitePool>,
}

// All queries filter by session_id
async fn save_knowledge(&self, args: Value) -> Result<MCPResult, String> {
    let session_id = args.get("session_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing session_id")?;

    sqlx::query(
        "INSERT INTO knowledge (id, session_id, ...) VALUES (?, ?, ...)"
    )
    .bind(id)
    .bind(session_id) // ← Session isolation
    .execute(self.db_pool.as_ref())
    .await?;
}

async fn search_knowledge(&self, args: Value) -> Result<MCPResult, String> {
    let session_id = args.get("session_id")...;

    sqlx::query(
        "SELECT * FROM knowledge WHERE session_id = ?" // ← Filter by session
    )
    .bind(session_id)
    .fetch_all(self.db_pool.as_ref())
    .await?;
}
```

### 3.3. Tool Execution Flow

```
User Request
     ↓
Agent System (via proxy_manager.execute_tool())
     ↓
MCPServiceProxy.call_tool()
     ↓
BuiltinMCPServer.call_tool() ← Routed to specific server
     ↓
Server-specific handler (e.g., create_assistant)
     ↓
Database / File System / Process Manager
     ↓
MCPResult (success or error)
```

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1. AssistantServer 완성 기준

**기능 요구사항:**

- ✅ create_assistant(): Duplicate handling, validation
- ✅ update_assistant(): Partial update support
- ✅ delete_assistant(): Cascade policy 정의
- ✅ list_assistants(): Pagination (limit/offset)
- ✅ get_assistant(): Improved error messages

**검증 방법:**

```rust
#[tokio::test]
async fn test_assistant_crud_lifecycle() {
    // Create
    let result = server.create_assistant(json!({
        "id": "test-1",
        "name": "Test Assistant",
        "config": {"model": "gpt-4"}
    })).await?;

    // Read
    let assistant = server.get_assistant(json!({"id": "test-1"})).await?;

    // Update
    server.update_assistant(json!({
        "id": "test-1",
        "config": {"model": "gpt-4-turbo"}
    })).await?;

    // List
    let list = server.list_assistants(json!({"limit": 10})).await?;
    assert_eq!(list.content[0].text.contains("test-1"), true);

    // Delete
    server.delete_assistant(json!({"id": "test-1"})).await?;
}
```

### 4.2. ContentStoreServer 완성 기준

**기능 요구사항:**

- ✅ Storage: Session-scoped content isolation
- ✅ All handlers pass session_id to storage
- ✅ Search engine respects session boundaries
- ✅ Content ID uniqueness per session (not global)

**검증 방법:**

```rust
#[tokio::test]
async fn test_content_store_session_isolation() {
    let server_a = ContentStoreServer::new(session_manager_a);
    let server_b = ContentStoreServer::new(session_manager_b);

    // Add content to session A
    server_a.handle_add_content(json!({
        "content": "Session A content",
        "metadata": {"title": "A"}
    })).await?;

    // Verify session B cannot see it
    let list_b = server_b.handle_list_content(json!({})).await?;
    assert_eq!(list_b.content.len(), 0);

    // Add content to session B
    server_b.handle_add_content(json!({
        "content": "Session B content",
        "metadata": {"title": "B"}
    })).await?;

    // Verify session A still has only its own content
    let list_a = server_a.handle_list_content(json!({})).await?;
    assert_eq!(list_a.content.len(), 1);
}
```

### 4.3. WorkspaceServer 완성 기준

**기능 요구사항:**

- ✅ Integration test with MCPServiceProxyManager
- ✅ Session isolation verification (workspace directories)
- ✅ Process cleanup on session termination
- ✅ Context switching validation

**검증 방법:**

```rust
#[tokio::test]
async fn test_workspace_server_integration() {
    let proxy_manager = MCPServiceProxyManager::new(...);

    // Create two sessions
    let session_a = proxy_manager.create_proxy("session-a", tools).await?;
    let session_b = proxy_manager.create_proxy("session-b", tools).await?;

    // Write file in session A
    proxy_manager.execute_tool("session-a", ToolCall {
        name: "writeFile",
        arguments: json!({"path": "test.txt", "content": "A"})
    }).await?;

    // Verify session B's workspace doesn't have it
    let result = proxy_manager.execute_tool("session-b", ToolCall {
        name: "readFile",
        arguments: json!({"path": "test.txt"})
    }).await;

    assert!(result.is_err() || result.unwrap().content[0].text.contains("not found"));
}
```

---

## 5. 수정이 필요한 코드 및 코드 스니핏

### 5.1. AssistantServer CRUD 완성

**파일**: `src-tauri/src/mcp/builtin/assistant/mod.rs`

#### A. create_assistant() 개선

```rust
async fn create_assistant(&self, args: Value) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'id' parameter".to_string())?;
    let name = args.get("name").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'name' parameter".to_string())?;
    let config = args.get("config")
        .ok_or_else(|| "Missing 'config' parameter".to_string())?;

    // Validate config is a valid JSON object
    if !config.is_object() {
        return Err("Config must be a JSON object".to_string());
    }

    let config_str = serde_json::to_string(config)
        .map_err(|e| format!("Invalid config JSON: {}", e))?;
    let now = chrono::Utc::now().timestamp_millis();

    let result = sqlx::query(
        r#"
        INSERT INTO assistants (id, name, config, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(&config_str)
    .bind(now)
    .bind(now)
    .execute(self.db_pool.as_ref())
    .await;

    match result {
        Ok(_) => Ok(MCPResult {
            content: vec![MCPContent::Text {
                text: format!("Assistant '{}' created successfully", id),
            }],
        }),
        Err(sqlx::Error::Database(db_err))
            if db_err.message().contains("UNIQUE constraint") => {
            Err(format!("Assistant with id '{}' already exists", id))
        }
        Err(e) => Err(format!("Failed to create assistant: {}", e)),
    }
}
```

#### B. update_assistant() 구현

```rust
async fn update_assistant(&self, args: Value) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'id' parameter".to_string())?;

    // Build dynamic UPDATE query based on provided fields
    let mut updates = Vec::new();
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
        updates.push("name = ?");
        bind_values.push(name.to_string());
    }

    if let Some(config) = args.get("config") {
        if !config.is_object() {
            return Err("Config must be a JSON object".to_string());
        }
        let config_str = serde_json::to_string(config)
            .map_err(|e| format!("Invalid config JSON: {}", e))?;
        updates.push("config = ?");
        bind_values.push(config_str);
    }

    if updates.is_empty() {
        return Err("No fields to update (provide 'name' or 'config')".to_string());
    }

    updates.push("updated_at = ?");
    let now = chrono::Utc::now().timestamp_millis();

    let query_str = format!(
        "UPDATE assistants SET {} WHERE id = ?",
        updates.join(", ")
    );

    let mut query = sqlx::query(&query_str);
    for value in bind_values {
        query = query.bind(value);
    }
    query = query.bind(now).bind(id);

    let result = query.execute(self.db_pool.as_ref()).await
        .map_err(|e| format!("Failed to update assistant: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Assistant '{}' not found", id));
    }

    Ok(MCPResult {
        content: vec![MCPContent::Text {
            text: format!("Assistant '{}' updated successfully", id),
        }],
    })
}
```

#### C. delete_assistant() 구현

```rust
async fn delete_assistant(&self, args: Value) -> Result<MCPResult, String> {
    let id = args.get("id").and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'id' parameter".to_string())?;

    let result = sqlx::query("DELETE FROM assistants WHERE id = ?")
        .bind(id)
        .execute(self.db_pool.as_ref())
        .await
        .map_err(|e| format!("Failed to delete assistant: {}", e))?;

    if result.rows_affected() == 0 {
        return Err(format!("Assistant '{}' not found", id));
    }

    Ok(MCPResult {
        content: vec![MCPContent::Text {
            text: format!("Assistant '{}' deleted successfully", id),
        }],
    })
}
```

#### D. list_assistants() 개선 (Pagination)

```rust
async fn list_assistants(&self, args: Value) -> Result<MCPResult, String> {
    let limit = args.get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(100) as i32;
    let offset = args.get("offset")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let assistants = sqlx::query_as::<_, (String, String, String, i64, i64)>(
        "SELECT id, name, config, created_at, updated_at
         FROM assistants
         ORDER BY updated_at DESC
         LIMIT ? OFFSET ?"
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(self.db_pool.as_ref())
    .await
    .map_err(|e| format!("Failed to list assistants: {}", e))?;

    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM assistants")
        .fetch_one(self.db_pool.as_ref())
        .await
        .unwrap_or(0);

    let json_array: Vec<Value> = assistants
        .into_iter()
        .map(|(id, name, config_str, created_at, updated_at)| {
            let config: Value = serde_json::from_str(&config_str).unwrap_or(json!({}));
            json!({
                "id": id,
                "name": name,
                "config": config,
                "created_at": created_at,
                "updated_at": updated_at
            })
        })
        .collect();

    Ok(MCPResult {
        content: vec![MCPContent::Text {
            text: serde_json::to_string_pretty(&json!({
                "total": count,
                "limit": limit,
                "offset": offset,
                "assistants": json_array
            }))
            .unwrap(),
        }],
    })
}
```

### 5.2. ContentStoreServer Session Isolation

**파일**: `src-tauri/src/mcp/builtin/content_store/server.rs`

#### A. Constructor 수정

```rust
impl ContentStoreServer {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        let session_id = session_manager.get_current_session()
            .expect("Session ID must be set");
        let session_dir = session_manager.get_session_workspace_dir();
        let search_index_dir = session_dir.join("content_store_search");

        let search_engine = search::ContentSearchEngine::new(search_index_dir)
            .expect("Failed to initialize search engine");

        let storage = storage::ContentStoreStorage::new_session_scoped(session_id.clone());

        Self {
            session_manager,
            storage: Mutex::new(storage),
            search_engine: Arc::new(Mutex::new(search_engine)),
        }
    }
}
```

**파일**: `src-tauri/src/mcp/builtin/content_store/storage.rs`

#### B. Storage 수정

```rust
pub struct ContentStoreStorage {
    session_id: String, // ← ADD THIS
    entries: HashMap<String, ContentEntry>,
}

impl ContentStoreStorage {
    pub fn new_session_scoped(session_id: String) -> Self {
        Self {
            session_id,
            entries: HashMap::new(),
        }
    }

    pub fn add(&mut self, entry: ContentEntry) -> Result<(), String> {
        // Content ID는 session 내에서만 unique하면 됨
        if self.entries.contains_key(&entry.id) {
            return Err(format!("Content '{}' already exists in this session", entry.id));
        }
        self.entries.insert(entry.id.clone(), entry);
        Ok(())
    }

    pub fn list(&self) -> Vec<&ContentEntry> {
        // 자동으로 session-scoped (self.entries만 반환)
        self.entries.values().collect()
    }

    pub fn get(&self, id: &str) -> Option<&ContentEntry> {
        self.entries.get(id)
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        self.entries.remove(id)
            .ok_or_else(|| format!("Content '{}' not found", id))?;
        Ok(())
    }
}
```

**파일**: `src-tauri/src/mcp/builtin/content_store/handlers.rs`

#### C. Handler 수정 (session_id 제거)

```rust
// ❌ Before: session_id를 args에서 받음
pub async fn handle_add_content(&self, args: Value) -> Result<MCPResult, String> {
    let session_id = args.get("session_id")...;
    // ...
}

// ✅ After: session_id는 이미 storage에 있음
pub async fn handle_add_content(&self, args: Value) -> Result<MCPResult, String> {
    // session_id 인자 불필요 - storage가 이미 session-scoped
    let content = args.get("content")...;
    let metadata = args.get("metadata")...;

    let entry = ContentEntry {
        id: uuid::Uuid::new_v4().to_string(),
        content: content.to_string(),
        metadata,
        created_at: chrono::Utc::now(),
    };

    self.storage.lock().await.add(entry)?;
    // ...
}
```

### 5.3. WorkspaceServer Integration Tests

**파일**: `src-tauri/tests/workspace_integration_tests.rs` (New)

```rust
use libragent::mcp::builtin::workspace::WorkspaceServer;
use libragent::mcp::builtin::BuiltinMCPServer;
use libragent::session::SessionManager;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_workspace_builtin_trait_integration() {
    let session_manager = Arc::new(SessionManager::new("test-session-1".to_string()));
    let server = WorkspaceServer::new(session_manager);

    // Test trait methods
    assert_eq!(server.name(), "workspace");
    assert!(server.tools().len() > 10);

    let context = server.get_service_context(None);
    assert!(context.context_prompt.contains("Workspace"));
}

#[tokio::test]
async fn test_workspace_session_isolation() {
    let session_a = Arc::new(SessionManager::new("session-a".to_string()));
    let session_b = Arc::new(SessionManager::new("session-b".to_string()));

    let server_a = WorkspaceServer::new(session_a);
    let server_b = WorkspaceServer::new(session_b);

    // Write file in session A
    let result_a = server_a.call_tool("writeFile", json!({
        "path": "test.txt",
        "content": "Session A content"
    })).await;
    assert!(result_a.is_ok());

    // Try to read from session B (should fail or return different workspace)
    let result_b = server_b.call_tool("readFile", json!({
        "path": "test.txt"
    })).await;

    // Should either error or read from different workspace directory
    assert!(
        result_b.is_err() ||
        !result_b.unwrap().content[0].text.contains("Session A content")
    );
}

#[tokio::test]
async fn test_workspace_context_switching() {
    let session_manager = Arc::new(SessionManager::new("test-session".to_string()));
    let server = WorkspaceServer::new(session_manager.clone());

    // Switch context
    let result = server.switch_context(ServiceContextOptions {
        session_id: Some("new-session".to_string()),
    }).await;

    assert!(result.is_ok());

    // Verify context changed
    let context = server.get_service_context(None);
    assert!(context.structured_state.is_some());
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1. KnowledgeServer (Session-Scoped Pattern 참조)

**파일**: `src-tauri/src/mcp/builtin/knowledge/mod.rs`

**주요 기능:**

- Session-scoped database queries with `WHERE session_id = ?`
- CRUD operations with session isolation
- Error handling patterns

**재사용 패턴:**

```rust
// Query pattern
let results = sqlx::query_as::<_, (String, String)>(
    "SELECT id, content FROM knowledge WHERE session_id = ?"
)
.bind(session_id)
.fetch_all(self.db_pool.as_ref())
.await?;

// Insert pattern
sqlx::query(
    "INSERT INTO knowledge (id, session_id, content) VALUES (?, ?, ?)"
)
.bind(id)
.bind(session_id)
.bind(content)
.execute(self.db_pool.as_ref())
.await?;
```

### 6.2. BuiltinMCPServer Trait

**파일**: `src-tauri/src/mcp/builtin/mod.rs`

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn display_name(&self) -> String { self.name().to_string() }
    fn tools(&self) -> Vec<MCPTool>;
    fn get_service_context(&self, options: Option<&Value>) -> ServiceContext;
    async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String>;
    async fn call_tool(&self, tool_name: &str, args: Value) -> Result<MCPResult, String>;
}
```

### 6.3. SessionManager

**파일**: `src-tauri/src/session/mod.rs`

**주요 메서드:**

```rust
pub fn get_current_session(&self) -> Option<String>;
pub fn get_session_workspace_dir(&self) -> PathBuf;
pub fn switch_session(&mut self, session_id: String) -> Result<(), String>;
```

**사용 예시:**

```rust
let session_id = session_manager.get_current_session()
    .ok_or("No active session")?;
let workspace_dir = session_manager.get_session_workspace_dir();
```

---

## 7. Test Code 추가 및 수정 가이드

### 7.1. AssistantServer Unit Tests

**파일**: `src-tauri/src/mcp/builtin/assistant/tests.rs` (New)

**테스트 시나리오:**

1. `test_create_assistant_success` - 정상 생성
2. `test_create_assistant_duplicate` - Duplicate key 에러
3. `test_update_assistant_partial` - Partial update (name만 변경)
4. `test_update_assistant_not_found` - 존재하지 않는 ID
5. `test_delete_assistant_success` - 정상 삭제
6. `test_delete_assistant_not_found` - 존재하지 않는 ID
7. `test_list_assistants_pagination` - Pagination 동작
8. `test_get_assistant_success` - 단일 조회

### 7.2. ContentStoreServer Session Isolation Tests

**파일**: `src-tauri/src/mcp/builtin/content_store/test_session_isolation.rs` (New)

**테스트 시나리오:**

1. `test_content_isolation_between_sessions` - 세션 간 content 격리
2. `test_content_id_uniqueness_per_session` - 같은 ID 다른 세션에서 사용 가능
3. `test_search_respects_session_boundaries` - 검색이 세션 경계 존중
4. `test_concurrent_sessions_content_operations` - 동시 세션 content 작업

### 7.3. WorkspaceServer Integration Tests

**파일**: `src-tauri/tests/workspace_integration_tests.rs`

**테스트 시나리오:**

1. `test_workspace_builtin_trait_integration` - Trait 통합
2. `test_workspace_session_isolation` - 워크스페이스 격리
3. `test_workspace_context_switching` - Context 전환
4. `test_workspace_process_cleanup` - 프로세스 cleanup
5. `test_workspace_with_proxy_manager` - ProxyManager 통합

---

## 8. 추가 분석 과제

### 8.1. AssistantServer Foreign Key 관계

**분석 필요 사항:**

- Assistant 삭제 시 Sessions table에 영향 있는지 확인
- CASCADE 전략 vs RESTRICT 전략 선택
- 현재 Sessions table의 assistants 컬럼 구조 파악

**제안:**

- 초기 구현: Soft delete (deleted_at column 추가)
- 추후 개선: Foreign key constraint + CASCADE

### 8.2. ContentStoreServer Storage Backend 활용

**분석 필요 사항:**

- ✅ SQLite persistent storage Rust Built-in에 이미 구현됨
- Session별 독립 storage: `new_with_sqlite()`에서 session_id 전달 방법
- Storage 크기 제한 및 cleanup 정책

**제안:**

- 초기 구현: **기존 Rust SQLite 코드 활용** (`new_with_sqlite()` 사용)
- Session isolation: Storage queries에 `WHERE session_id = ?` 추가
- 추후 개선: Content 크기 제한, LRU cleanup policy

### 8.3. WorkspaceServer Process Lifecycle

**분석 필요 사항:**

- Session 종료 시 running process 처리 방식
- Graceful shutdown vs Forced kill
- Orphan process detection 메커니즘

**제안:**

- 초기 구현: Session 종료 시 SIGTERM 전송, 5초 후 SIGKILL
- 추후 개선: User-configurable timeout, graceful shutdown hooks

---

## 9. Clarification Q-List

### Q1. AssistantServer Deletion Policy

**질문:** Assistant 삭제 시 해당 assistant를 사용하는 기존 sessions는 어떻게 처리할 것인가?

**옵션:**

- **A:** Hard delete 허용 (sessions는 invalid assistant reference 가짐)
- **B:** Soft delete (deleted_at 컬럼, sessions는 계속 참조 가능)
- **C:** Delete 거부 (active sessions가 있으면 삭제 불가)

**현재 제안:** B (Soft delete) - 기존 sessions 영향 없음, 추후 cleanup task로 정리

> 답변: Option B 적용

---

### Q2. WorkspaceServer Process Cleanup Timing

**질문:** Session 종료 시 running processes를 언제 cleanup 할 것인가?

**옵션:**

- **A:** 즉시 cleanup (session terminate 시 모든 프로세스 kill)
- **B:** Graceful period (5초 대기 후 cleanup)
- **C:** 24시간 retention (현재 구현, background task가 정리)

**현재 제안:** B (Graceful period) - 프로세스가 graceful shutdown 할 기회 제공

> 답변: B 선택

---

### Q3. Test Coverage 목표

**질문:** 각 서버의 테스트 커버리지 목표는?

**옵션:**

- **A:** 80%+ (핵심 기능만)
- **B:** 90%+ (대부분의 코드)
- **C:** 95%+ (거의 모든 코드)

**현재 제안:** B (90%+) - 핵심 기능 + 에러 케이스 + 통합 테스트

> 답변: B 선택

---

## 10. Implementation Timeline

### Phase 1: AssistantServer (2 days)

- Day 1: CRUD operations 완성, unit tests
- Day 2: Integration tests, documentation

### Phase 2: ContentStoreServer (3 days)

- Day 1-2: Session isolation refactoring, storage 수정
- Day 3: Session isolation tests, documentation

### Phase 3: WorkspaceServer (2 days)

- Day 1: Integration tests 작성
- Day 2: Proxy manager 통합 검증, documentation

### Phase 4: Final Integration (1 day)

- All servers together testing
- Performance verification
- Documentation update

**Total Estimated Time**: 8 days
