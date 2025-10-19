# Tool Implementation Guide: Web MCP vs Rust Backend

기존의 `switch_context` 기반 설계에서 파라미터 기반 설계로 전환할 때, 각 backend type별 도구 구현 방식이 달라집니다.

---

## Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│ Frontend: ChatContext, useAIService, useToolProcessor           │
├─────────────────────────────────────────────────────────────────┤
│ All tool calls get __sessionId, __assistantId injected          │
└──────┬───────────────────────────────────┬──────────────────────┘
       │                                   │
       ▼                                   ▼
┌─────────────────────┐        ┌──────────────────────────┐
│ Web MCP Servers     │        │ Rust Built-in Servers    │
├─────────────────────┤        ├──────────────────────────┤
│ planning-server     │        │ content_store            │
│ playbook-store      │        │ workspace                │
│ ui-tools            │        │ (other servers)          │
└─────────────────────┘        └──────────────────────────┘
```

---

## 1. Web MCP Servers (TypeScript)

**위치**: `src/lib/web-mcp/modules/`

### 특징

- **실행 환경**: Web Worker (따라서 동기/비동기 자유도 높음)
- **상태 관리**: 메모리 내 JavaScript 객체 (Map, Set, etc.)
- **격리**: Worker 프로세스별로 독립적 상태 (하지만 refactoring 후는 session key로 격리)
- **개발 난이도**: 낮음 (TypeScript, 친숙한 패턴)

### 1.1 General Pattern

```typescript
// src/lib/web-mcp/modules/planning-server.ts

// ─────────────────────────────────────────────────────────────
// 1. Context 추출 Helper (모든 Web MCP 서버 공통)
// ─────────────────────────────────────────────────────────────

function extractContext(args: Record<string, unknown>): {
  sessionId: string;
  assistantId?: string;
  threadId?: string;
} {
  return {
    sessionId: (args.__sessionId as string) || 'default',
    assistantId: args.__assistantId as string | undefined,
    threadId: args.__threadId as string | undefined,
  };
}

function getStateKey(
  sessionId: string,
  assistantId?: string,
  threadId?: string,
): string {
  // 상태 저장소의 key 생성 (unique identifier)
  return `${sessionId}::${assistantId || 'none'}::${threadId || 'default'}`;
}

// ─────────────────────────────────────────────────────────────
// 2. 상태 저장소 (session/assistant/thread별 격리)
// ─────────────────────────────────────────────────────────────

// Before: globalState 하나 사용
// const stateManager = new SessionStateManager();

// After: context key별 상태 격리
const ephemeralStateMap = new Map<string, EphemeralState>();

function getOrCreateState(stateKey: string): EphemeralState {
  if (!ephemeralStateMap.has(stateKey)) {
    ephemeralStateMap.set(stateKey, new EphemeralState());
  }
  return ephemeralStateMap.get(stateKey)!;
}

// ─────────────────────────────────────────────────────────────
// 3. Helper: Args 정제 (__ prefix 필드 제거)
// ─────────────────────────────────────────────────────────────

function cleanArgs(args: Record<string, unknown>): Record<string, unknown> {
  // AI가 생성한 실제 args만 남김 (infrastructure fields 제거)
  const cleaned = { ...args };
  delete cleaned.__sessionId;
  delete cleaned.__assistantId;
  delete cleaned.__threadId;
  return cleaned;
}

// ─────────────────────────────────────────────────────────────
// 4. Server Implementation
// ─────────────────────────────────────────────────────────────

const planningServer: WebMCPServer = {
  // switchContext 제거! ✅
  // async switchContext(...) { ... }  ← 이제 필요 없음

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const typedArgs = args as Record<string, unknown>;

    // Step 1: Context 추출 (middleware에서 주입됨)
    const context = extractContext(typedArgs);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId,
    );

    // Step 2: Context별 상태 가져오기
    const state = getOrCreateState(stateKey);

    // Step 3: 실제 tool args 정제 (__ prefix 제거)
    const cleanedArgs = cleanArgs(typedArgs);

    // Step 4: Tool 실행
    switch (name) {
      case 'create_goal': {
        const goal = cleanedArgs.goal as string;
        if (!goal) {
          return createMCPTextResponse('Error: goal is required');
        }
        return state.createGoal(goal);
      }

      case 'add_todo': {
        const todoName = cleanedArgs.name as string;
        const goalId = cleanedArgs.goal_id as string | undefined;
        return state.addTodo(todoName, goalId);
      }

      case 'mark_todo': {
        const todoId = cleanedArgs.todo_id as string;
        return state.markTodo(todoId);
      }

      case 'get_planning_state': {
        return state.getPlanningState();
      }

      default:
        return createMCPTextResponse(`Unknown tool: ${name}`);
    }
  },
};

export { planningServer };
```

### 1.2 Practical Example: playbook-store

```typescript
// src/lib/web-mcp/modules/playbook-store.ts

// ─────────────────────────────────────────────────────────────
// 상태 관리 (session/assistant별로 격리된 playbook store)
// ─────────────────────────────────────────────────────────────

interface PlaybookStoreState {
  playbookRecords: Map<string, PlaybookRecord>;
}

function getOrCreatePlaybookStore(stateKey: string): PlaybookStoreState {
  if (!playbookStoreMap.has(stateKey)) {
    playbookStoreMap.set(stateKey, {
      playbookRecords: new Map(),
    });
  }
  return playbookStoreMap.get(stateKey)!;
}

const playbookStoreMap = new Map<string, PlaybookStoreState>();

// ─────────────────────────────────────────────────────────────
// Server Implementation
// ─────────────────────────────────────────────────────────────

const playbookStore: WebMCPServer = {
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const typedArgs = args as Record<string, unknown>;

    // Context 추출 (assistant 소유권 검사 등에 사용)
    const context = extractContext(typedArgs);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId,
    );

    const store = getOrCreatePlaybookStore(stateKey);
    const cleanedArgs = cleanArgs(typedArgs);

    switch (name) {
      case 'select_playbook': {
        const playbookId = cleanedArgs.id as string;

        const playbook = store.playbookRecords.get(playbookId);
        if (!playbook) {
          return createMCPTextResponse(`Playbook not found: ${playbookId}`);
        }

        // ⚠️ 권한 검사: context.assistantId와 비교
        if (context.assistantId && playbook.agentId !== context.assistantId) {
          return createMCPTextResponse(
            `Error: Playbook ${playbookId} does not belong to assistant ${context.assistantId}`,
          );
        }

        return createMCPJSONResponse({ selected: playbookId });
      }

      case 'create_playbook': {
        const name = cleanedArgs.name as string;
        const agentId = context.assistantId || 'unknown';

        const newPlaybook: PlaybookRecord = {
          id: generateId(),
          name,
          agentId,
          steps: [],
        };

        store.playbookRecords.set(newPlaybook.id, newPlaybook);
        return createMCPJSONResponse(newPlaybook);
      }

      case 'list_playbooks': {
        // ⚠️ 현재 assistant에 속한 playbook만 반환
        const myPlaybooks = Array.from(store.playbookRecords.values()).filter(
          (pb) =>
            context.assistantId ? pb.agentId === context.assistantId : true,
        );

        return createMCPJSONResponse({
          playbooks: myPlaybooks,
        });
      }

      default:
        return createMCPTextResponse(`Unknown tool: ${name}`);
    }
  },
};

export { playbookStore };
```

### 1.3 Memory Management (중요!)

Web MCP는 **메모리 누적** 가능성이 있으므로 cleanup 전략이 필수:

```typescript
// ─────────────────────────────────────────────────────────────
// Memory Management Strategy
// ─────────────────────────────────────────────────────────────

// Option A: TTL 기반 (권장)
interface StateWithMetadata {
  state: EphemeralState;
  lastAccessedAt: number;
}

const ephemeralStateMap = new Map<string, StateWithMetadata>();

function getOrCreateState(stateKey: string): EphemeralState {
  const existing = ephemeralStateMap.get(stateKey);
  const now = Date.now();

  if (existing) {
    existing.lastAccessedAt = now;
    return existing.state;
  }

  const newState = new EphemeralState();
  ephemeralStateMap.set(stateKey, {
    state: newState,
    lastAccessedAt: now,
  });
  return newState;
}

// Cleanup: 1시간 미접근 상태 제거
setInterval(
  () => {
    const now = Date.now();
    const TTL_MS = 60 * 60 * 1000; // 1 hour

    for (const [key, metadata] of ephemeralStateMap.entries()) {
      if (now - metadata.lastAccessedAt > TTL_MS) {
        ephemeralStateMap.delete(key);
        // Optional: 로깅
        console.log(`[planningServer] Cleaned up state: ${key}`);
      }
    }
  },
  5 * 60 * 1000,
); // 5분마다 cleanup 체크

// Option B: LRU Cache (고급)
// - 최대 N개 상태만 메모리에 유지
// - 오래된 상태 자동 제거
import LRU from 'lru-cache';

const lruCache = new LRU<string, EphemeralState>({
  max: 100, // 최대 100개 session-assistant-thread 조합
  ttl: 1000 * 60 * 60, // 1 hour TTL
});
```

### 1.4 Web MCP Server Checklist

```typescript
// ✅ 필수 구현 사항

interface WebMCPServerImplementation {
  // 1. Context 추출
  extractContext(args: Record<string, unknown>): ContextInfo;

  // 2. State key 생성
  getStateKey(context: ContextInfo): string;

  // 3. Args 정제
  cleanArgs(args: Record<string, unknown>): Record<string, unknown>;

  // 4. State 저장소 (Map 기반)
  stateMap: Map<string, ServerState>;

  // 5. Tool 핸들러 (switch문)
  async callTool(name: string, args: unknown): Promise<MCPResponse>;

  // 6. 메모리 관리 (TTL 또는 LRU)
  setupCleanup(): void;
}

// ❌ 제거할 것
// - switchContext 메서드
// - 전역 상태 변수 (currentSessionId, currentAssistantId)
// - SessionStateManager.setSession() 호출
```

---

## 2. Rust Built-in Servers

**위치**: `src-tauri/src/mcp/builtin/`

### Characteristics

- **실행 환경**: Tokio async runtime (고성능, thread-safe)
- **상태 관리**: Arc<Mutex<>> 또는 Arc<RwLock<>> (thread-safe)
- **격리**: Store 내 session ID를 key로 사용 (예: IndexedMap, HashMap)
- **개발 난이도**: 높음 (Rust, lifetime, async/await)

### 2.1 General Pattern

```rust
// src-tauri/src/mcp/builtin/content_store/server.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::{json, Value};

// ─────────────────────────────────────────────────────────────
// 1. Context 추출 Helper
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,
}

impl ToolContext {
    /// args에서 __ prefix 필드 추출
    pub fn from_args(args: &Value) -> Self {
        let session_id = args
            .get("__sessionId")
            .or_else(|| args.get("__session_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let assistant_id = args
            .get("__assistantId")
            .or_else(|| args.get("__assistant_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        let thread_id = args
            .get("__threadId")
            .or_else(|| args.get("__thread_id"))
            .and_then(|v| v.as_str())
            .map(String::from);

        Self {
            session_id,
            assistant_id,
            thread_id,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 2. Args 정제 Helper
// ─────────────────────────────────────────────────────────────

pub fn clean_args(args: &Value) -> Value {
    // JSON에서 __ prefix 필드 제거
    let mut cleaned = args.clone();

    if let Some(obj) = cleaned.as_object_mut() {
        obj.remove("__sessionId");
        obj.remove("__session_id");
        obj.remove("__assistantId");
        obj.remove("__assistant_id");
        obj.remove("__threadId");
        obj.remove("__thread_id");
    }

    cleaned
}

// ─────────────────────────────────────────────────────────────
// 3. Server Structure (Session-scoped storage)
// ─────────────────────────────────────────────────────────────

pub struct ContentStoreServer {
    /// Session별 저장소 (key: session_id)
    storage: Arc<Mutex<HashMap<String, SessionContentStore>>>,
}

struct SessionContentStore {
    session_id: String,
    /// 실제 content 저장 (session 내에서 격리)
    contents: Vec<ContentItem>,
    /// Semantic search index
    bm25_index: BM25Index,
}

impl ContentStoreServer {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Session-scoped store 가져오기 (또는 생성)
    async fn get_or_create_session_store(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<SessionContentStore>>, String> {
        let mut storage = self.storage.lock().await;

        if !storage.contains_key(session_id) {
            storage.insert(
                session_id.to_string(),
                SessionContentStore {
                    session_id: session_id.to_string(),
                    contents: Vec::new(),
                    bm25_index: BM25Index::new(),
                },
            );
        }

        Ok(Arc::new(Mutex::new(
            storage
                .get(session_id)
                .unwrap()
                .clone(),
        )))
    }
}

// ─────────────────────────────────────────────────────────────
// 4. Tool Handlers
// ─────────────────────────────────────────────────────────────

#[async_trait::async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "content_store"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "add_content".to_string(),
                description: "Add content to session store".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["filename", "content"]
                }),
            },
            // ... other tools
        ]
    }

    // ✅ switchContext 메서드 제거!
    // Before:
    // async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> { ... }
    // After: 없음!

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        // Step 1: Context 추출 (middleware에서 주입됨)
        let context = ToolContext::from_args(&args);

        // Step 2: Args 정제
        let cleaned = clean_args(&args);

        // Step 3: Session-scoped store 접근
        match self.get_or_create_session_store(&context.session_id).await {
            Ok(session_store) => {
                match tool_name {
                    "add_content" => {
                        self.handle_add_content(
                            &context,
                            &cleaned,
                            session_store,
                        )
                        .await
                    }

                    "read_content" => {
                        self.handle_read_content(
                            &context,
                            &cleaned,
                            session_store,
                        )
                        .await
                    }

                    "search_content" => {
                        self.handle_search_content(
                            &context,
                            &cleaned,
                            session_store,
                        )
                        .await
                    }

                    _ => create_error_response(-32601, &format!("Unknown tool: {}", tool_name)),
                }
            }
            Err(e) => create_error_response(-32603, &e),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 5. Individual Tool Handlers (session context 명시적 사용)
// ─────────────────────────────────────────────────────────────

impl ContentStoreServer {
    async fn handle_add_content(
        &self,
        context: &ToolContext,
        args: &Value,
        session_store: Arc<Mutex<SessionContentStore>>,
    ) -> MCPResponse {
        let filename = args
            .get("filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "filename is required".to_string());

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "content is required".to_string());

        match (filename, content) {
            (Ok(f), Ok(c)) => {
                let mut store = session_store.lock().await;

                // ✅ Session-scoped: 현재 session만 영향 받음
                store.contents.push(ContentItem {
                    id: generate_id(),
                    filename: f.to_string(),
                    content: c.to_string(),
                    session_id: context.session_id.clone(),
                    created_at: chrono::Utc::now(),
                });

                create_success_response(json!({
                    "status": "added",
                    "filename": f
                }))
            }
            _ => create_error_response(-32602, "Invalid arguments"),
        }
    }

    async fn handle_search_content(
        &self,
        context: &ToolContext,
        args: &Value,
        session_store: Arc<Mutex<SessionContentStore>>,
    ) -> MCPResponse {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut store = session_store.lock().await;

        // ✅ Session-scoped: 현재 session의 content만 검색
        let results: Vec<_> = store
            .contents
            .iter()
            .filter(|c| {
                c.session_id == context.session_id
                    && (c.filename.contains(query) || c.content.contains(query))
            })
            .collect();

        create_success_response(json!({
            "results": results,
            "count": results.len()
        }))
    }
}
```

### 2.2 Practical Example: workspace

```rust
// src-tauri/src/mcp/builtin/workspace/mod.rs

pub struct WorkspaceServer {
    /// Session별 process registry (key: session_id)
    process_registry: Arc<Mutex<HashMap<String, SessionProcesses>>>,
}

struct SessionProcesses {
    session_id: String,
    processes: HashMap<String, ProcessInfo>,
}

impl WorkspaceServer {
    async fn handle_execute_command(
        &self,
        context: &ToolContext,
        args: &Value,
        _: (),
    ) -> MCPResponse {
        let command = match args.get("command").and_then(|v| v.as_str()) {
            Some(cmd) => cmd,
            None => return create_error_response(-32602, "command is required"),
        };

        let mut registry = self.process_registry.lock().await;

        // ✅ Session-scoped: 현재 session의 process만 생성
        let session_procs = registry
            .entry(context.session_id.clone())
            .or_insert_with(|| SessionProcesses {
                session_id: context.session_id.clone(),
                processes: HashMap::new(),
            });

        let process_id = generate_id();
        let process_info = ProcessInfo {
            id: process_id.clone(),
            command: command.to_string(),
            session_id: context.session_id.clone(),
            started_at: chrono::Utc::now(),
            // ...
        };

        session_procs.processes.insert(process_id.clone(), process_info);

        create_success_response(json!({
            "processId": process_id,
            "status": "started"
        }))
    }

    async fn handle_poll_process(
        &self,
        context: &ToolContext,
        args: &Value,
        _: (),
    ) -> MCPResponse {
        let process_id = match args.get("processId").and_then(|v| v.as_str()) {
            Some(id) => id,
            None => return create_error_response(-32602, "processId is required"),
        };

        let registry = self.process_registry.lock().await;

        // ✅ Session-scoped access control
        match registry.get(&context.session_id) {
            Some(session_procs) => match session_procs.processes.get(process_id) {
                Some(proc_info) => {
                    // ✅ 보안: 같은 session 내에서만 접근 가능
                    if proc_info.session_id != context.session_id {
                        return create_error_response(
                            -32603,
                            "Access denied: process belongs to different session",
                        );
                    }

                    create_success_response(json!({
                        "processId": process_id,
                        "status": "running",
                        "output": "" // ... get actual output
                    }))
                }
                None => create_error_response(-32602, "Process not found"),
            },
            None => create_error_response(-32602, "No processes in this session"),
        }
    }
}
```

### 2.3 Rust Server Checklist

```rust
// ✅ 필수 구현 사항

pub trait BuiltinMCPServerImpl {
    // 1. Context 추출 (ToolContext::from_args)
    fn extract_context(args: &Value) -> ToolContext;

    // 2. Args 정제 (clean_args)
    fn clean_args(args: &Value) -> Value;

    // 3. Session-scoped 저장소
    // Arc<Mutex<HashMap<String, SessionState>>>

    // 4. get_or_create_session_store
    async fn get_or_create_session_store(
        &self,
        session_id: &str,
    ) -> Result<Arc<Mutex<SessionState>>, String>;

    // 5. Tool 핸들러 (match statement)
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;

    // 6. Individual handler methods
    async fn handle_* (
        &self,
        context: &ToolContext,
        args: &Value,
    ) -> MCPResponse;
}

// ❌ 제거할 것
// - switch_context 메서드 (trait에서 제거)
// - SessionManager.get_current_session() 호출
// - 전역 session state 의존
```

---

## 3. Comparison: Web MCP vs Rust Backend

| 항목               | Web MCP             | Rust Backend             |
| ------------------ | ------------------- | ------------------------ |
| **언어**           | TypeScript          | Rust                     |
| **상태 저장**      | Map (string, State) | Arc Mutex HashMap        |
| **메모리 관리**    | TTL/LRU 필수        | GC 자동                  |
| **Concurrency**    | Worker 격리         | Tokio async/await        |
| **Context 추출**   | extractContext()    | ToolContext::from_args() |
| **Args 정제**      | cleanArgs()         | clean_args()             |
| **Session 격리**   | stateKey 기반       | HashMap 키 기반          |
| **Access Control** | 코드 내 검사        | context 기반             |

---

## 4. Common Patterns

### 4.1 Context-Based Permission Checking

```typescript
// Web MCP (TypeScript)
case 'select_playbook': {
  const context = extractContext(typedArgs);
  const playbook = store.playbookRecords.get(playbookId);

  // 권한 검사: context.assistantId와 비교
  if (context.assistantId && playbook.agentId !== context.assistantId) {
    return createMCPTextResponse('Access denied');
  }
  // ...
}
```

```rust
// Rust Backend
async fn handle_select_playbook(
    &self,
    context: &ToolContext,
    args: &Value,
) -> MCPResponse {
    let playbook_id = /* ... */;

    // 권한 검사: context.assistant_id와 비교
    if let Some(assistant_id) = &context.assistant_id {
        if playbook.agent_id != *assistant_id {
            return create_error_response(-32603, "Access denied");
        }
    }
    // ...
}
```

### 4.2 Session-Scoped State Access

```typescript
// Web MCP (TypeScript)
const stateKey = getStateKey(
  context.sessionId,
  context.assistantId,
  context.threadId,
);
const state = getOrCreateState(stateKey);

// state는 이 context에만 격리됨
state.addTodo(name);
```

```rust
// Rust Backend
let session_store = self
    .get_or_create_session_store(&context.session_id)
    .await?;

let mut store = session_store.lock().await;
// store는 이 session에만 격리됨
store.contents.push(content_item);
```

### 4.3 Error Handling

```typescript
// Web MCP (TypeScript)
if (!goal) {
  return createMCPTextResponse('Error: goal is required');
}

return state.createGoal(goal);
```

```rust
// Rust Backend
let goal = args
    .get("goal")
    .and_then(|v| v.as_str())
    .ok_or_else(|| "goal is required".to_string())?;

Ok(state.create_goal(goal))
```

---

## 5. Migration Checklist

### For Web MCP Servers

- [ ] Remove `switchContext` method from `WebMCPServer` interface
- [ ] Add `extractContext()` helper
- [ ] Add `getStateKey()` function
- [ ] Add `cleanArgs()` helper
- [ ] Replace global state with `Map<string, State>`
- [ ] Update all tool handlers to:
  1. Extract context
  2. Get state by key
  3. Clean args
  4. Execute tool logic
- [ ] Add memory management (TTL or LRU)
- [ ] Update tool definitions (verify no context fields)

### For Rust Servers

- [ ] Remove `switch_context` method from trait
- [ ] Add `ToolContext::from_args()` helper
- [ ] Add `clean_args()` function
- [ ] Change storage to `Arc<Mutex<HashMap<session_id, State>>>`
- [ ] Add `get_or_create_session_store()` method
- [ ] Update all tool handlers to:
  1. Extract context
  2. Get session-scoped store
  3. Clean args
  4. Verify access control (session_id match)
  5. Execute tool logic
- [ ] Update tool definitions (verify no context fields)
- [ ] Test concurrent requests to different sessions

### For Frontend

- [ ] Update `use-tool-processor.ts` to inject `__sessionId`, `__assistantId`, `__threadId`
- [ ] Verify tool invocation always includes these fields
- [ ] Verify tool definitions remain unchanged
- [ ] Test end-to-end tool call flow

---

## 6. Testing Strategy

### Web MCP Tests

```typescript
describe('planning-server', () => {
  it('should isolate state by session', async () => {
    const server = createPlanningServer();

    // Session 1
    await server.callTool('create_goal', {
      goal: 'Goal A',
      __sessionId: 'sess_1',
    });

    // Session 2
    await server.callTool('create_goal', {
      goal: 'Goal B',
      __sessionId: 'sess_2',
    });

    // Verify isolation
    const state1 = getOrCreateState(getStateKey('sess_1'));
    const state2 = getOrCreateState(getStateKey('sess_2'));

    expect(state1.goals).toContainEqual(
      expect.objectContaining({ name: 'Goal A' }),
    );
    expect(state2.goals).toContainEqual(
      expect.objectContaining({ name: 'Goal B' }),
    );
    expect(state1.goals).not.toContainEqual(
      expect.objectContaining({ name: 'Goal B' }),
    );
  });

  it('should enforce permission checks on assistantId', async () => {
    const server = createPlaybookStore();

    await server.callTool('create_playbook', {
      name: 'PB1',
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });

    const result = await server.callTool('select_playbook', {
      id: 'pb_1',
      __sessionId: 'sess_1',
      __assistantId: 'asst_2', // Different assistant
    });

    expect(result.error).toBeDefined();
    expect(result.error.message).toContain('not belong to assistant');
  });
});
```

### Rust Tests

```rust
#[tokio::test]
async fn test_session_isolation() {
    let server = ContentStoreServer::new();

    // Session 1
    server.call_tool("add_content", json!({
        "filename": "file1.txt",
        "content": "Session 1 content",
        "__sessionId": "sess_1"
    })).await;

    // Session 2
    server.call_tool("add_content", json!({
        "filename": "file2.txt",
        "content": "Session 2 content",
        "__sessionId": "sess_2"
    })).await;

    // Verify isolation
    let storage = server.storage.lock().await;
    assert_eq!(storage.get("sess_1").unwrap().contents.len(), 1);
    assert_eq!(storage.get("sess_2").unwrap().contents.len(), 1);
}
```

---

## 7. FAQ

**Q: \_\_ prefix가 Tool Definition에 나타나면?**
A: 안 됨! Tool schema에서 제거해야 함. 이는 AI가 알 필요 없는 infrastructure field.

**Q: Session 종료 시 정리는?**
A: Web MCP는 TTL/LRU로 자동 정리. Rust는 필요 시 explicit cleanup signal (e.g., `on_session_end` hook) 구현.

**Q: 같은 session/assistant에 대한 동시 요청?**
A: 가능! `Arc<Mutex<>>` (Rust) 또는 Map (Web MCP)로 thread-safe 보장.

**Q: assistantId 없는 요청?**
A: `context.assistantId.is_none()`으로 처리. Permission check는 optional.

**Q: Backward compatibility?**
A: `__sessionId` 없으면 'default' 사용. 로깅으로 모니터링 권장.
