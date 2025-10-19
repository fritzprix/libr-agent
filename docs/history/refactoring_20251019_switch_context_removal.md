# Refactoring Plan: Switch Context Removal and Parameter-Based Session Management

**작성일**: 2025-10-19  
**작업 범위**: MCP Tool Context 관리 아키텍처 전환  
**우선순위**: High (동시 요청 처리 안정성 확보)

---

## 1. 작업의 목적

### 1.1 문제 정의

현재 `switch_context` 기반 설계는 **stateful backend operation**으로 다중 요청 처리에 부적합합니다:

- **Race Condition**: 전역 상태(`currentSessionId`, `currentAssistantId`)에 의존하여 동시 요청 시 context 충돌 발생
- **확장성 제약**: Multi-agent, multi-session 시나리오 지원 불가
- **Debugging 어려움**: 요청별 context 추적 불가능
- **Architecture 복잡도**: `switch_context` async operation 필수, 호출 순서 의존성

### 1.2 목표

1. **`switch_context` 완전 제거**: 모든 Web MCP 및 Rust Backend에서 제거
2. **Parameter-based Context 도입**: 각 tool 호출마다 `__sessionId`, `__assistantId`, `__threadId` 파라미터 전달
3. **Tool Definition 분리**: AI에게 노출되는 schema에 context fields 미포함 (middleware 투명 주입)
4. **동시 요청 안전성**: Session/assistant/thread별 state 완전 격리
5. **호환성 유지**: 기존 Tool Definition 변경 없음, 점진적 마이그레이션 가능

---

## 2. 현재의 상태 / 문제점

### 2.1 Web MCP Servers 현황

#### planning-server.ts (`src/lib/web-mcp/modules/planning-server.ts`)

**현재 구조**:

```typescript
class SessionStateManager {
  private sessions = new Map<string, EphemeralState>();
  private currentSessionId: string | null = null; // ⚠️ 전역 상태

  setSession(sessionId: string): void {
    this.currentSessionId = sessionId;
  }

  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) throw new Error('No session');
    return this.sessions.get(this.currentSessionId)!;
  }
}

const planningServer: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    if (context.sessionId) {
      stateManager.setSession(context.sessionId);
    }
  },

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // ⚠️ getCurrentState()에 의존 - 전역 상태 사용
    const state = stateManager.getCurrentState();
    // ...
  },
};
```

**문제점**:

- 동시 요청 A (session1) → `switchContext(session2)` → 요청 B 실행 시 B가 session2로 실행됨
- `assistantId`, `threadId` 미지원
- State 격리 불가능

#### playbook-store.ts (`src/lib/web-mcp/modules/playbook-store.ts`)

**현재 구조**:

```typescript
let currentAssistantId: string | null = null; // ⚠️ 전역 변수

const playbookStore: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    currentAssistantId = context.assistantId ?? null;
  },

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // ⚠️ 권한 검사가 전역 currentAssistantId에 의존
    if (currentAssistantId && existing.agentId !== currentAssistantId) {
      return createMCPTextResponse('Error: Permission denied');
    }
  },
};
```

**문제점**:

- 동시 요청 시 권한 검사가 잘못된 assistant에 대해 수행될 수 있음
- Session 격리 없음

### 2.2 Rust Backend 현황

#### content_store (`src-tauri/src/mcp/builtin/content_store/server.rs`)

**현재 구조**:

```rust
pub struct ContentStoreServer {
    pub(crate) session_manager: Arc<SessionManager>,  // ⚠️ 싱글톤
    pub(crate) storage: Mutex<ContentStoreStorage>,
}

pub async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
    if let Some(session_id) = &options.session_id {
        self.session_manager.set_current_session(session_id.clone()).await?;
    }
    Ok(())
}

pub async fn handle_add_content(&self, args: Value) -> MCPResponse {
    // ⚠️ 전역 SessionManager에 의존
    let session_id = self.session_manager.get_current_session()?;
    // ...
}
```

**문제점**:

- `SessionManager`는 앱 전체 싱글톤
- 다른 thread에서 `switch_context` 호출 시 race condition
- Tool handler 호출 전 반드시 `switch_context` 호출 필요 (순서 의존성)

#### workspace (`src-tauri/src/mcp/builtin/workspace/mod.rs`)

**현재 구조**:

```rust
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,  // ⚠️ 싱글톤
    process_registry: ProcessRegistry,
}

pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager
        .get_current_session()
        .map_err(|e| format!("No session: {}", e))?;
    // ...
}
```

**문제점**:

- `session_manager.get_current_session()` 의존
- Process registry가 session별로 격리되지 않음

### 2.3 Frontend 현황

#### use-tool-processor.ts (`src/hooks/use-tool-processor.ts`)

**현재**:

```typescript
export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  // Tool 호출 시 sessionId 등을 전달하지 않음
  const response = await executeToolCall({
    id: toolCallId,
    name: toolCall.function.name,
    arguments: JSON.parse(toolCall.function.arguments),
  });
};
```

**문제점**:

- Context 주입 로직 없음
- Backend가 `switch_context`에 의존하도록 강제

---

## 3. 관련 코드의 구조 및 동작 방식 Summary (Bird's Eye View)

### 3.1 Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│ Frontend Layer                                                   │
├─────────────────────────────────────────────────────────────────┤
│ ChatContext → useAIService → useToolProcessor                   │
│  ├─ AI generates tool calls                                     │
│  ├─ executeToolCall() invokes backend                           │
│  └─ Currently: NO context injection                             │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│ Backend Layer (Current - Stateful)                              │
├─────────────────────────────────────────────────────────────────┤
│ 1. Frontend calls switchContext(sessionId)                      │
│ 2. Backend sets global state (currentSessionId)                 │
│ 3. Frontend calls callTool(name, args)                          │
│ 4. Backend reads global state to get session                    │
│ ⚠️ Problem: Steps 3-4 can be interleaved by concurrent requests │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow (Current)

```text
User Input → AI Model → Tool Calls
                          │
                          ▼
                    switchContext(sessionId)
                          │
                          ▼
                    [Global State Update]
                          │
                          ▼
                    callTool(name, args)
                          │
                          ▼
                    [Read Global State] ⚠️ Race condition here!
                          │
                          ▼
                    Tool Execution
```

### 3.3 Critical Design Principle: Tool Definition Separation

**⚠️ Most Important Constraint**:

```text
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: AI (sees Tool Definition - JSON Schema)                │
├─────────────────────────────────────────────────────────────────┤
│ {                                                                │
│   "name": "create_goal",                                        │
│   "inputSchema": {                                              │
│     "properties": { "goal": { "type": "string" } }              │
│   }                                                             │
│ }                                                               │
│ ✅ NO sessionId, assistantId, threadId                          │
└──────────────────────┬──────────────────────────────────────────┘
                       │ (AI generates args)
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: Frontend Middleware (use-tool-processor.ts)            │
├─────────────────────────────────────────────────────────────────┤
│ Input:  { "goal": "Learn Rust" }                                │
│ Output: {                                                       │
│   "goal": "Learn Rust",                                         │
│   "__sessionId": "sess_123",     ← Injected transparently       │
│   "__assistantId": "asst_456",   ← Injected transparently       │
│   "__threadId": "thread_789"     ← Injected transparently       │
│ }                                                               │
│ ✅ __ prefix marks infrastructure fields (AI invisible)         │
└──────────────────────┬──────────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 3: Backend (Web MCP / Rust)                               │
├─────────────────────────────────────────────────────────────────┤
│ 1. Extract: { sessionId, assistantId, threadId } from args      │
│ 2. Remove: Delete __sessionId, __assistantId, __threadId        │
│ 3. Process: Use clean args { "goal": "Learn Rust" }             │
│ 4. Isolate: Use extracted context for state lookup              │
└─────────────────────────────────────────────────────────────────┘
```

**Key Rules**:

1. ✅ **DO**: Inject `__sessionId`, etc. in middleware layer (frontend)
2. ✅ **DO**: Extract and use in backend tool handlers
3. ✅ **DO**: Remove \_\_ fields before processing business logic
4. ❌ **DON'T**: Include in Tool Definition (JSON schema)
5. ❌ **DON'T**: Let AI know about these fields

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1 목표 아키텍처

```text
User Input → AI Model → Tool Calls
                          │
                          ▼
                    [Middleware: Inject Context]
                          │
                          ▼
                    callTool(name, args + __sessionId + __assistantId)
                          │
                          ▼
                    [Backend: Extract Context from Args]
                          │
                          ▼
                    [State Lookup by Context Key]
                          │
                          ▼
                    Tool Execution (Isolated)
```

### 4.2 성공 판정 기준

#### 기능적 요구사항

- ✅ `switch_context` 완전 제거 (모든 Web MCP, Rust Backend)
- ✅ 동시 요청 처리 시 context 충돌 없음
- ✅ Session/assistant/thread별 state 완전 격리
- ✅ Tool Definition에 context fields 미포함
- ✅ 기존 Tool Definition 변경 없음 (AI 호환성 유지)

#### 비기능적 요구사항

- ✅ 성능 저하 없음 (parameter passing overhead 무시 가능)
- ✅ 메모리 누수 없음 (Web MCP TTL cleanup 동작)
- ✅ Debug 용이성 향상 (각 호출에 context 명시)
- ✅ 코드 복잡도 감소 (`switch_context` 호출 순서 의존성 제거)

#### 테스트 기준

- ✅ 단일 세션 테스트 통과
- ✅ 다중 세션 동시 실행 테스트 통과
- ✅ 동일 세션 동시 요청 테스트 통과
- ✅ Permission check (assistantId 기반) 정상 작동
- ✅ TTL cleanup (Web MCP) 정상 작동
- ✅ `pnpm refactor:validate` 통과 (lint, format, build)

---

## 5. 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 5.1 Phase 1: Type & Interface 검증

#### Frontend: ServiceContextOptions (이미 존재)

**파일**: `src/features/tools/index.tsx`

```typescript
// ✅ 현재 이미 정의되어 있음 - 변경 불필요
export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string; // 확인 필요
}
```

**작업**: `threadId` 필드 존재 여부 확인, 없으면 추가

#### Backend Rust: ServiceContextOptions

**파일**: `src-tauri/src/mcp/types.rs`

```rust
// 현재
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContextOptions {
    pub session_id: Option<String>,
    pub assistant_id: Option<String>,
    // thread_id 확인 필요
}

// 변경 후 (필요시)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContextOptions {
    pub session_id: Option<String>,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,  // ← 추가
}
```

### 5.2 Phase 2: Frontend Middleware

#### use-tool-processor.ts

**파일**: `src/hooks/use-tool-processor.ts`

**Before**:

```typescript
export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  // ...
  const response = await executeToolCall({
    id: toolCallId,
    name: toolCall.function.name,
    arguments: JSON.parse(toolCall.function.arguments),
  });
};
```

**After**:

```typescript
export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  // Context 가져오기 (ChatContext 또는 props에서)
  const sessionId = getCurrentSessionId(); // 구현 필요
  const assistantId = getCurrentAssistantId(); // 구현 필요
  const threadId = getCurrentThreadId(); // Optional

  // AI args에 context 주입
  const aiArgs = JSON.parse(toolCall.function.arguments);
  const argsWithContext = {
    ...aiArgs, // AI 생성 args 보존
    __sessionId: sessionId,
    __assistantId: assistantId,
    __threadId: threadId,
  };

  const response = await executeToolCall({
    id: toolCallId,
    name: toolCall.function.name,
    arguments: argsWithContext, // ← Context 포함
  });
};
```

### 5.3 Phase 3: Web MCP Servers

#### planning-server.ts

**파일**: `src/lib/web-mcp/modules/planning-server.ts`

**Before**:

```typescript
class SessionStateManager {
  private sessions = new Map<string, EphemeralState>();
  private currentSessionId: string | null = null;

  setSession(sessionId: string): void {
    this.currentSessionId = sessionId;
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, new EphemeralState());
    }
  }

  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) {
      throw new Error('No session set');
    }
    return this.sessions.get(this.currentSessionId)!;
  }
}

const stateManager = new SessionStateManager();

const planningServer: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    if (context.sessionId) {
      stateManager.setSession(context.sessionId);
    }
  },

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const state = stateManager.getCurrentState();
    // ...
  },
};
```

**After**:

```typescript
// ─────────────────────────────────────────────────────────────
// Helper Functions
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
  return `${sessionId}::${assistantId || 'none'}::${threadId || 'default'}`;
}

function cleanArgs(args: Record<string, unknown>): Record<string, unknown> {
  const cleaned = { ...args };
  delete cleaned.__sessionId;
  delete cleaned.__assistantId;
  delete cleaned.__threadId;
  return cleaned;
}

// ─────────────────────────────────────────────────────────────
// State Management
// ─────────────────────────────────────────────────────────────

interface StateWithMetadata {
  state: EphemeralState;
  lastAccessedAt: number;
}

const ephemeralStateMap = new Map<string, StateWithMetadata>();

function getOrCreateState(stateKey: string): EphemeralState {
  const now = Date.now();
  const existing = ephemeralStateMap.get(stateKey);

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

// TTL Cleanup (1시간 미접근 상태 제거)
const TTL_MS = 60 * 60 * 1000;
setInterval(
  () => {
    const now = Date.now();
    for (const [key, entry] of ephemeralStateMap.entries()) {
      if (now - entry.lastAccessedAt > TTL_MS) {
        ephemeralStateMap.delete(key);
      }
    }
  },
  5 * 60 * 1000,
); // 5분마다 체크

// ─────────────────────────────────────────────────────────────
// Server Implementation
// ─────────────────────────────────────────────────────────────

const planningServer: WebMCPServer = {
  // ✅ switchContext 제거

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const typedArgs = args as Record<string, unknown>;

    // Context 추출
    const context = extractContext(typedArgs);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId,
    );

    // State 조회/생성
    const state = getOrCreateState(stateKey);

    // Args 정제
    const cleanedArgs = cleanArgs(typedArgs);

    // Tool 실행
    switch (name) {
      case 'create_goal': {
        const goal = cleanedArgs.goal as string;
        return state.createGoal(goal);
      }
      // ... other tools
    }
  },
};

export { planningServer };
```

#### playbook-store.ts

**파일**: `src/lib/web-mcp/modules/playbook-store.ts`

**Before**:

```typescript
let currentAssistantId: string | null = null;

const playbookStore: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    currentAssistantId = context.assistantId ?? null;
  },

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    case 'select_playbook': {
      if (currentAssistantId && existing.agentId !== currentAssistantId) {
        return createMCPTextResponse('Error: Permission denied');
      }
    }
  },
};
```

**After**:

```typescript
// currentAssistantId 전역 변수 제거

const playbookStoreMap = new Map<string, PlaybookStoreState>();

const playbookStore: WebMCPServer = {
  // ✅ switchContext 제거

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const typedArgs = args as Record<string, unknown>;

    // Context 추출
    const context = extractContext(typedArgs);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId,
    );

    // State 조회/생성
    const store = getOrCreatePlaybookStore(stateKey);

    // Args 정제
    const cleanedArgs = cleanArgs(typedArgs);

    switch (name) {
      case 'select_playbook': {
        const id = cleanedArgs.id as string;
        const existing = store.playbookRecords.get(id);

        // Permission check using context.assistantId
        if (
          context.assistantId &&
          existing &&
          existing.agentId !== context.assistantId
        ) {
          return createMCPTextResponse('Error: Permission denied');
        }

        // ... proceed
      }
    }
  },
};
```

### 5.4 Phase 4: Rust Backend

#### content_store/server.rs

**파일**: `src-tauri/src/mcp/builtin/content_store/server.rs`

**Before**:

```rust
pub struct ContentStoreServer {
    pub(crate) session_manager: Arc<SessionManager>,
    pub(crate) storage: Mutex<ContentStoreStorage>,
}

pub async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
    if let Some(session_id) = &options.session_id {
        self.session_manager.set_current_session(session_id.clone()).await?;
    }
    Ok(())
}

pub async fn handle_add_content(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager.get_current_session()?;
    // ...
}
```

**After**:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::Value;

// ─────────────────────────────────────────────────────────────
// Context Extraction
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,
}

impl ToolContext {
    pub fn from_args(args: &Value) -> Self {
        let session_id = args
            .get("__sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let assistant_id = args
            .get("__assistantId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let thread_id = args
            .get("__threadId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Self {
            session_id,
            assistant_id,
            thread_id,
        }
    }
}

pub fn clean_args(args: &Value) -> Value {
    let mut cleaned = args.clone();
    if let Some(obj) = cleaned.as_object_mut() {
        obj.remove("__sessionId");
        obj.remove("__assistantId");
        obj.remove("__threadId");
    }
    cleaned
}

// ─────────────────────────────────────────────────────────────
// Session-Scoped Storage
// ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SessionContentStore {
    pub session_id: String,
    pub contents: Vec<ContentItem>,
    pub bm25_index: BM25Index,
}

pub struct ContentStoreServer {
    // ✅ session_manager 제거
    storage: Arc<Mutex<HashMap<String, SessionContentStore>>>,
}

impl ContentStoreServer {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_or_create_session_store(
        &self,
        session_id: &str,
    ) -> SessionContentStore {
        let mut storage = self.storage.lock().await;

        storage
            .entry(session_id.to_string())
            .or_insert_with(|| SessionContentStore {
                session_id: session_id.to_string(),
                contents: Vec::new(),
                bm25_index: BM25Index::new(),
            })
            .clone()
    }

    async fn handle_add_content(
        &self,
        context: ToolContext,
        args: &Value,
    ) -> MCPResponse {
        let cleaned = clean_args(args);
        let filename = cleaned.get("filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "filename is required".to_string())?;

        // Session store 조회
        let mut store = self.get_or_create_session_store(&context.session_id).await;

        // Content 추가
        store.contents.push(ContentItem {
            filename: filename.to_string(),
            content: /* ... */,
        });

        // Update storage
        let mut storage = self.storage.lock().await;
        storage.insert(context.session_id.clone(), store);

        Ok(/* response */)
    }
}

#[async_trait::async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "content_store"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            MCPTool {
                name: "add_content".to_string(),
                description: "Add content to store".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {
                        "filename": { "type": "string" },
                        "content": { "type": "string" }
                        // ✅ NO __sessionId, __assistantId
                    },
                    "required": ["filename", "content"]
                }),
            },
            // ... other tools
        ]
    }

    // ✅ switch_context 제거

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        // Context 추출
        let context = ToolContext::from_args(&args);

        match tool_name {
            "add_content" => self.handle_add_content(context, &args).await,
            // ... other tools
            _ => Err(format!("Unknown tool: {}", tool_name)),
        }
    }
}
```

#### workspace/mod.rs

**파일**: `src-tauri/src/mcp/builtin/workspace/mod.rs`

**Before**:

```rust
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,
    process_registry: ProcessRegistry,
}

pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager.get_current_session()?;
    // ...
}
```

**After**:

```rust
pub struct WorkspaceServer {
    // ✅ session_manager 제거
    process_registry: Arc<Mutex<HashMap<String, SessionProcesses>>>,
}

struct SessionProcesses {
    session_id: String,
    processes: HashMap<String, ProcessInfo>,
}

impl WorkspaceServer {
    async fn handle_execute_command(
        &self,
        context: ToolContext,
        args: &Value,
    ) -> MCPResponse {
        let cleaned = clean_args(args);
        let command = cleaned.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "command is required".to_string())?;

        // Session-scoped process registry
        let mut registry = self.process_registry.lock().await;
        let session_processes = registry
            .entry(context.session_id.clone())
            .or_insert_with(|| SessionProcesses {
                session_id: context.session_id.clone(),
                processes: HashMap::new(),
            });

        // Execute and register process
        let process_id = generate_process_id();
        session_processes.processes.insert(process_id.clone(), /* ... */);

        Ok(/* response */)
    }

    async fn handle_poll_process(
        &self,
        context: ToolContext,
        args: &Value,
    ) -> MCPResponse {
        let cleaned = clean_args(args);
        let process_id = cleaned.get("processId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "processId is required".to_string())?;

        // Session-scoped lookup
        let registry = self.process_registry.lock().await;
        let session_processes = registry
            .get(&context.session_id)
            .ok_or_else(|| "No processes for this session".to_string())?;

        let process = session_processes.processes
            .get(process_id)
            .ok_or_else(|| format!("Process {} not found", process_id))?;

        Ok(/* response */)
    }
}
```

### 5.5 Phase 5: Trait Changes

#### BuiltinMCPServer trait

**파일**: `src-tauri/src/mcp/builtin/mod.rs`

**Before**:

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;

    async fn switch_context(
        &self,
        options: ServiceContextOptions,
    ) -> Result<(), String>;

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

**After**:

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;

    // ✅ switch_context 제거

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1 공통 Helper Functions (TypeScript)

**위치**: `src/lib/web-mcp/common/context-helpers.ts` (새로 생성)

```typescript
/**
 * Extract context from tool args
 */
export function extractContext(args: Record<string, unknown>): {
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

/**
 * Generate state key for session/assistant/thread isolation
 */
export function getStateKey(
  sessionId: string,
  assistantId?: string,
  threadId?: string,
): string {
  return `${sessionId}::${assistantId || 'none'}::${threadId || 'default'}`;
}

/**
 * Remove infrastructure fields from args
 */
export function cleanArgs(
  args: Record<string, unknown>,
): Record<string, unknown> {
  const cleaned = { ...args };
  delete cleaned.__sessionId;
  delete cleaned.__assistantId;
  delete cleaned.__threadId;
  return cleaned;
}

/**
 * TTL-based cleanup for state maps
 */
export function createTTLCleanup<T>(
  stateMap: Map<string, { state: T; lastAccessedAt: number }>,
  ttlMs: number = 60 * 60 * 1000,
  intervalMs: number = 5 * 60 * 1000,
): NodeJS.Timeout {
  return setInterval(() => {
    const now = Date.now();
    for (const [key, entry] of stateMap.entries()) {
      if (now - entry.lastAccessedAt > ttlMs) {
        stateMap.delete(key);
      }
    }
  }, intervalMs);
}
```

### 6.2 공통 Helper Functions (Rust)

**위치**: `src-tauri/src/mcp/builtin/common.rs` (새로 생성)

```rust
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,
}

impl ToolContext {
    /// Extract context from tool args
    pub fn from_args(args: &Value) -> Self {
        let session_id = args
            .get("__sessionId")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let assistant_id = args
            .get("__assistantId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let thread_id = args
            .get("__threadId")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Self {
            session_id,
            assistant_id,
            thread_id,
        }
    }
}

/// Remove infrastructure fields from args
pub fn clean_args(args: &Value) -> Value {
    let mut cleaned = args.clone();
    if let Some(obj) = cleaned.as_object_mut() {
        obj.remove("__sessionId");
        obj.remove("__assistantId");
        obj.remove("__threadId");
    }
    cleaned
}
```

### 6.3 재사용 가능한 인터페이스

**파일**: `src/models/mcp-types.ts`

```typescript
export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string;
}

export interface MCPTool {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

export interface MCPResponse<T = unknown> {
  content: Array<{
    type: string;
    text: string;
  }>;
  isError?: boolean;
}
```

**파일**: `src-tauri/src/mcp/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContextOptions {
    pub session_id: Option<String>,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}
```

---

## 7. Test Code 추가 및 수정 필요 부분에 대한 가이드

### 7.1 Web MCP Tests

**파일**: `src/lib/web-mcp/modules/__tests__/planning-server.test.ts` (새로 생성)

```typescript
import { describe, it, expect } from 'vitest';
import { planningServer } from '../planning-server';

describe('planningServer - Parameter-based Context', () => {
  it('should isolate state by session', async () => {
    // Session 1: Create goal
    const response1 = await planningServer.callTool('create_goal', {
      goal: 'Session 1 Goal',
      __sessionId: 'sess_1',
    });
    expect(response1.isError).toBe(false);

    // Session 2: Create goal
    const response2 = await planningServer.callTool('create_goal', {
      goal: 'Session 2 Goal',
      __sessionId: 'sess_2',
    });
    expect(response2.isError).toBe(false);

    // Session 1: List goals (should only have sess_1 goal)
    const list1 = await planningServer.callTool('list_goals', {
      __sessionId: 'sess_1',
    });
    expect(list1.content[0].text).toContain('Session 1 Goal');
    expect(list1.content[0].text).not.toContain('Session 2 Goal');
  });

  it('should isolate state by assistant within same session', async () => {
    const response1 = await planningServer.callTool('create_goal', {
      goal: 'Assistant 1 Goal',
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });

    const response2 = await planningServer.callTool('create_goal', {
      goal: 'Assistant 2 Goal',
      __sessionId: 'sess_1',
      __assistantId: 'asst_2',
    });

    // Check isolation
    const list1 = await planningServer.callTool('list_goals', {
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });
    expect(list1.content[0].text).toContain('Assistant 1 Goal');
    expect(list1.content[0].text).not.toContain('Assistant 2 Goal');
  });

  it('should handle concurrent requests safely', async () => {
    const promises = [
      planningServer.callTool('create_goal', {
        goal: 'Concurrent 1',
        __sessionId: 'sess_1',
      }),
      planningServer.callTool('create_goal', {
        goal: 'Concurrent 2',
        __sessionId: 'sess_2',
      }),
      planningServer.callTool('create_goal', {
        goal: 'Concurrent 3',
        __sessionId: 'sess_3',
      }),
    ];

    const results = await Promise.all(promises);
    expect(results.every((r) => !r.isError)).toBe(true);

    // Verify each session has only its own goal
    const list1 = await planningServer.callTool('list_goals', {
      __sessionId: 'sess_1',
    });
    const list2 = await planningServer.callTool('list_goals', {
      __sessionId: 'sess_2',
    });
    const list3 = await planningServer.callTool('list_goals', {
      __sessionId: 'sess_3',
    });

    expect(list1.content[0].text).toContain('Concurrent 1');
    expect(list2.content[0].text).toContain('Concurrent 2');
    expect(list3.content[0].text).toContain('Concurrent 3');
  });
});
```

**파일**: `src/lib/web-mcp/modules/__tests__/playbook-store.test.ts`

```typescript
describe('playbookStore - Permission Checks', () => {
  it('should allow access when assistantId matches', async () => {
    // Create playbook with assistant 1
    await playbookStore.callTool('create_playbook', {
      name: 'Test Playbook',
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });

    // Select playbook with same assistant (should succeed)
    const response = await playbookStore.callTool('select_playbook', {
      id: 'pb_123',
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });

    expect(response.isError).toBe(false);
  });

  it('should deny access when assistantId differs', async () => {
    // Create playbook with assistant 1
    await playbookStore.callTool('create_playbook', {
      name: 'Test Playbook',
      __sessionId: 'sess_1',
      __assistantId: 'asst_1',
    });

    // Try to select with different assistant (should fail)
    const response = await playbookStore.callTool('select_playbook', {
      id: 'pb_123',
      __sessionId: 'sess_1',
      __assistantId: 'asst_2',
    });

    expect(response.isError).toBe(true);
    expect(response.content[0].text).toContain('Permission denied');
  });
});
```

### 7.2 Rust Backend Tests

**파일**: `src-tauri/src/mcp/builtin/content_store/tests.rs` (새로 생성)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_session_isolation() {
        let server = ContentStoreServer::new();

        // Add content to session 1
        let args1 = json!({
            "filename": "test1.txt",
            "content": "Content 1",
            "__sessionId": "sess_1"
        });
        let result1 = server.call_tool("add_content", args1).await;
        assert!(result1.is_ok());

        // Add content to session 2
        let args2 = json!({
            "filename": "test2.txt",
            "content": "Content 2",
            "__sessionId": "sess_2"
        });
        let result2 = server.call_tool("add_content", args2).await;
        assert!(result2.is_ok());

        // List content in session 1 (should only have test1.txt)
        let list_args1 = json!({ "__sessionId": "sess_1" });
        let list1 = server.call_tool("list_contents", list_args1).await.unwrap();
        let text1 = &list1.content[0].text;
        assert!(text1.contains("test1.txt"));
        assert!(!text1.contains("test2.txt"));

        // List content in session 2 (should only have test2.txt)
        let list_args2 = json!({ "__sessionId": "sess_2" });
        let list2 = server.call_tool("list_contents", list_args2).await.unwrap();
        let text2 = &list2.content[0].text;
        assert!(text2.contains("test2.txt"));
        assert!(!text2.contains("test1.txt"));
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let server = Arc::new(ContentStoreServer::new());

        let server1 = server.clone();
        let server2 = server.clone();
        let server3 = server.clone();

        let (result1, result2, result3) = tokio::join!(
            server1.call_tool("add_content", json!({
                "filename": "concurrent1.txt",
                "content": "Content 1",
                "__sessionId": "sess_1"
            })),
            server2.call_tool("add_content", json!({
                "filename": "concurrent2.txt",
                "content": "Content 2",
                "__sessionId": "sess_2"
            })),
            server3.call_tool("add_content", json!({
                "filename": "concurrent3.txt",
                "content": "Content 3",
                "__sessionId": "sess_3"
            }))
        );

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result3.is_ok());

        // Verify isolation
        let storage = server.storage.lock().await;
        assert_eq!(storage.get("sess_1").unwrap().contents.len(), 1);
        assert_eq!(storage.get("sess_2").unwrap().contents.len(), 1);
        assert_eq!(storage.get("sess_3").unwrap().contents.len(), 1);
    }
}
```

**파일**: `src-tauri/src/mcp/builtin/workspace/tests.rs`

```rust
#[tokio::test]
async fn test_process_session_isolation() {
    let server = WorkspaceServer::new();

    // Execute command in session 1
    let args1 = json!({
        "command": "echo 'Session 1'",
        "__sessionId": "sess_1"
    });
    let result1 = server.call_tool("execute_command", args1).await.unwrap();
    let process_id_1 = extract_process_id(&result1);

    // Execute command in session 2
    let args2 = json!({
        "command": "echo 'Session 2'",
        "__sessionId": "sess_2"
    });
    let result2 = server.call_tool("execute_command", args2).await.unwrap();
    let process_id_2 = extract_process_id(&result2);

    // Poll process from session 1 (should succeed)
    let poll1 = server.call_tool("poll_process", json!({
        "processId": process_id_1,
        "__sessionId": "sess_1"
    })).await;
    assert!(poll1.is_ok());

    // Try to poll session 1's process from session 2 (should fail)
    let poll_cross = server.call_tool("poll_process", json!({
        "processId": process_id_1,
        "__sessionId": "sess_2"
    })).await;
    assert!(poll_cross.is_err());
}
```

### 7.3 Integration Tests

**파일**: `src/test/integration/context-management.test.ts`

```typescript
describe('End-to-End Context Management', () => {
  it('should maintain context through full chat flow', async () => {
    // 1. Create AI service with sessionId
    const aiService = createAIService({
      sessionId: 'integration_test_session',
      assistantId: 'integration_test_assistant',
    });

    // 2. Send message that triggers tool use
    const response = await aiService.sendMessage('Create a goal: Test E2E');

    // 3. Verify tool was called with correct context
    expect(response.toolCalls).toHaveLength(1);
    expect(response.toolCalls[0].arguments).toMatchObject({
      goal: 'Test E2E',
      __sessionId: 'integration_test_session',
      __assistantId: 'integration_test_assistant',
    });

    // 4. Verify state isolation
    const anotherService = createAIService({
      sessionId: 'another_session',
    });
    const anotherResponse = await anotherService.sendMessage('List my goals');
    expect(anotherResponse.text).not.toContain('Test E2E');
  });
});
```

---

## 8. 추가 분석 과제

### 8.1 현재 미확정 사항

1. **Frontend Context 공급원**:
   - `use-tool-processor`에서 `sessionId`, `assistantId`, `threadId`를 어디서 가져올 것인가?
   - `ChatContext`에서 제공? Props로 전달? Global state?
   - **조사 필요**: `ChatContext.tsx` 구조 분석

2. **Rust SessionManager 의존성**:
   - `content_store`, `workspace` 외에 `SessionManager`를 사용하는 다른 서버 있는지 조사
   - **조사 필요**: `src-tauri/src/mcp/builtin/` 전체 스캔

3. **Memory Management 전략**:
   - Web MCP TTL: 1시간이 적절한가? 사용 패턴 분석 필요
   - Rust: GC가 충분한가? Explicit cleanup 필요한가?
   - **조사 필요**: Production 메모리 사용량 모니터링

4. **Tool Definition 현황**:
   - 현재 어떤 tool들이 정의되어 있는가?
   - Context fields를 실수로 포함한 tool이 있는가?
   - **조사 필요**: 모든 `MCPTool` 정의 검토

### 8.2 Performance 분석 필요 사항

1. **Parameter Passing Overhead**:
   - `__sessionId` 등을 매 호출마다 전달하는 것의 overhead
   - **측정 필요**: Before/After 성능 비교 (tool call latency)

2. **State Lookup Performance**:
   - Map lookup (`ephemeralStateMap.get(stateKey)`) 성능
   - HashMap lookup (Rust) 성능
   - **측정 필요**: Concurrent request scenario에서 latency

3. **Memory Footprint**:
   - Session별 state 유지 시 메모리 사용량
   - **측정 필요**: 100 sessions, 1000 sessions 시뮬레이션

### 8.3 호환성 검증 필요 사항

1. **기존 Tool Calls**:
   - AI가 생성한 기존 tool call arguments가 그대로 작동하는가?
   - **검증 필요**: 프로덕션 로그에서 실제 tool call 샘플 추출하여 테스트

2. **Backward Compatibility**:
   - `switch_context` 제거 후 기존 코드가 graceful degradation 하는가?
   - **검증 필요**: Phase-by-phase rollout 시뮬레이션

3. **Multi-Agent Flow**:
   - Assistant A → Tool → Assistant B 같은 복잡한 flow 지원 가능한가?
   - **검증 필요**: Multi-agent orchestration 시나리오 테스트

---

## 9. 구현 순서 및 마이그레이션 전략

### Phase 1: Preparation (Week 1)

**목표**: Type 정의 검증 및 helper functions 준비

**작업**:

1. ✅ `ServiceContextOptions` interface 검토 (`threadId` 필드 확인)
2. ✅ Rust `ServiceContextOptions` struct 동기화
3. ✅ TypeScript helper functions 작성 (`src/lib/web-mcp/common/context-helpers.ts`)
4. ✅ Rust helper functions 작성 (`src-tauri/src/mcp/builtin/common.rs`)
5. ✅ Unit tests for helpers

**Validation**:

- All helpers have unit tests
- No breaking changes to existing types

### Phase 2: Frontend Middleware (Week 1-2)

**목표**: Context 주입 로직 구현

**작업**:

1. ✅ `use-tool-processor.ts` 분석 (context 공급원 파악)
2. ✅ Context injection 로직 추가
3. ✅ AI args 보존 확인 (untouched)
4. ✅ Integration test 작성

**Validation**:

- Tool calls include `__sessionId`, `__assistantId`
- AI-generated args unchanged
- No Tool Definition modifications

### Phase 3: Web MCP Servers (Week 2-3)

**목표**: `planning-server`, `playbook-store` 리팩토링

**작업**:

1. ✅ `planning-server.ts`:
   - `switchContext` 제거
   - Context 추출 로직 추가
   - State map 변경 (Map<stateKey, State>)
   - TTL cleanup 구현
   - Unit tests

2. ✅ `playbook-store.ts`:
   - `switchContext` 제거
   - `currentAssistantId` 전역 변수 제거
   - Permission check 리팩토링
   - Unit tests

**Validation**:

- All existing tests pass
- New tests for session isolation pass
- Concurrent request tests pass
- Memory cleanup working (manual verification)

### Phase 4: Rust Backend (Week 3-4)

**목표**: `content_store`, `workspace` 리팩토링

**작업**:

1. ✅ `BuiltinMCPServer` trait:
   - `switch_context` 메서드 제거

2. ✅ `content_store/server.rs`:
   - `SessionManager` 의존성 제거
   - Session-scoped storage 구현 (HashMap)
   - `ToolContext::from_args()` 사용
   - Tool handlers 리팩토링
   - Unit tests, integration tests

3. ✅ `workspace/mod.rs`:
   - `SessionManager` 제거
   - Session-scoped process registry 구현
   - Context 기반 process isolation
   - Unit tests

**Validation**:

- `cargo test` 통과
- Session isolation tests 통과
- Concurrent request tests 통과
- No `SessionManager` dependency

### Phase 5: Testing & Deployment (Week 4)

**목표**: 전체 시스템 검증 및 배포

**작업**:

1. ✅ End-to-end integration tests
2. ✅ Performance tests (tool call latency)
3. ✅ Memory leak tests (장시간 실행)
4. ✅ Concurrent request stress tests (100+ concurrent)
5. ✅ `pnpm refactor:validate` 통과
6. ✅ Code review
7. ✅ Staging deployment
8. ✅ Production deployment with monitoring

**Validation**:

- All tests pass
- No performance regression
- No memory leaks
- Production monitoring shows no errors
- Context isolation working in production

---

## 10. Risk Assessment & Mitigation

### 10.1 High Risk

**Risk**: Context injection 누락으로 인한 runtime error

**Mitigation**:

- Frontend middleware에서 default value 제공 (`'default'` session)
- Backend에서 graceful fallback (missing context → default)
- Logging으로 모니터링 (context 없이 호출된 경우 경고)

### 10.2 Medium Risk

**Risk**: Memory leak (Web MCP state 누적)

**Mitigation**:

- TTL cleanup 구현 필수
- Production monitoring으로 메모리 사용량 추적
- LRU cache 대안 고려 (고급)

### 10.3 Low Risk

**Risk**: Tool Definition 실수로 context fields 포함

**Mitigation**:

- Code review checklist에 명시
- Automated test로 검증 (Tool schema에 `__` prefix 필드 없는지 확인)
- Documentation 명확히 (Critical Design Principle 강조)

---

## 11. Success Metrics

### 11.1 Functional Metrics

- ✅ `switch_context` 완전 제거 (모든 서버)
- ✅ 단일 세션 테스트 100% 통과
- ✅ 다중 세션 동시 실행 테스트 100% 통과
- ✅ Permission check (assistantId 기반) 정상 작동

### 11.2 Non-Functional Metrics

- ✅ Tool call latency: Before 대비 5% 이내 (parameter passing overhead)
- ✅ Memory usage: 100 sessions에서 안정적 (누수 없음)
- ✅ Concurrent request: 100+ 동시 요청 처리 가능
- ✅ Code complexity: `switch_context` 제거로 async dependency 제거

### 11.3 Quality Metrics

- ✅ Test coverage: 신규 코드 80% 이상
- ✅ `pnpm refactor:validate` 통과
- ✅ Code review approval
- ✅ Documentation complete

---

## 12. References

### 12.1 관련 문서

- [`docs/refactoring/switch-context-removal-refactoring.md`](../refactoring/switch-context-removal-refactoring.md) - 전략 문서
- [`docs/refactoring/tool-implementation-guide.md`](../refactoring/tool-implementation-guide.md) - 구현 가이드
- [`docs/refactoring/before-after-examples.md`](../refactoring/before-after-examples.md) - 코드 예제
- [`docs/architecture/chat-feature-architecture.md`](../architecture/chat-feature-architecture.md) - Chat 아키텍처

### 12.2 핵심 파일 위치

**Frontend**:

- `src/hooks/use-tool-processor.ts` - Context injection
- `src/features/tools/index.tsx` - ServiceContextOptions interface
- `src/lib/web-mcp/modules/planning-server.ts` - Web MCP 예제
- `src/lib/web-mcp/modules/playbook-store.ts` - Web MCP 예제

**Backend**:

- `src-tauri/src/mcp/builtin/mod.rs` - BuiltinMCPServer trait
- `src-tauri/src/mcp/builtin/content_store/server.rs` - Rust 예제
- `src-tauri/src/mcp/builtin/workspace/mod.rs` - Rust 예제
- `src-tauri/src/mcp/types.rs` - ServiceContextOptions struct

### 12.3 Key Types

**TypeScript**:

```typescript
interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string;
}
```

**Rust**:

```rust
pub struct ToolContext {
    pub session_id: String,
    pub assistant_id: Option<String>,
    pub thread_id: Option<String>,
}
```

---

**작성자**: AI Architecture Team  
**검토자**: TBD  
**승인자**: TBD  
**최종 수정일**: 2025-10-19
