# Refactoring Strategy: Removing `switch_context` and Adding Parameter-Based Session Context

## Executive Summary

현재 `switch_context`는 **stateful backend operation**으로 다중 요청 처리에 부적합합니다.
각 도구가 상태를 전역으로 관리하며(e.g., `SessionStateManager`, `currentAssistantId`), 동시에 여러 AI 요청/세션이 들어오면 race condition이 발생합니다.

**해결 방안**:

1. `switch_context` 완전 제거
2. 모든 도구 호출(callTool)에 `sessionId`, `assistantId`, `threadId`를 **인자로 명시 전달**
3. 도구 구현에서 전역 상태 대신 **매 호출마다 파라미터로 받은 context 사용**
4. 상태 격리: session/assistant/thread별 독립적 저장소 관리---

## Current Problem Analysis

### 1. `planning-server.ts` (Web MCP)

**위치**: `src/lib/web-mcp/modules/planning-server.ts`

**현재 구조**:

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

  // 모든 도구는 getCurrentState()를 호출하여 currentSessionId에 의존
  addTodo(name: string): MCPResponse<AddToDoOutput> {
    return this.getCurrentState().addTodo(name);
  }
}

const planningServer: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    const sessionId = context.sessionId;
    if (sessionId) {
      stateManager.setSession(sessionId); // ⚠️ 전역 상태 변경
    }
  },
};
```

**문제점**:

- `switchContext` 호출 후 모든 도구가 같은 `currentSessionId`를 사용
- 동시 요청 A (session1) → switchContext(session2) → 요청 B (session1) 실행 시 B의 context가 session2로 변경됨
- `assistantId`는 아예 미지원 (planning에선 사용 안 함)

**파라미터 전달 필요**:

````typescript
**파라미터 전달 필요**:

```typescript
// Tool definition (AI가 보는 것 - 변경 없음)
{
  "name": "create_goal",
  "description": "Create a new goal",
  "inputSchema": {
    "type": "object",
    "properties": {
      "goal": { "type": "string" }
      // ← sessionId 없음! AI는 이것만 봄
    }
  }
}

// Tool invocation (backend에서 받는 것 - middleware 주입)
callTool('create_goal', {
  goal: 'Learn Rust',                    // AI가 생성
  __sessionId: 'sess_123',               // ← middleware 주입 (__ prefix)
  __assistantId: 'asst_456',             // ← middleware 주입 (__ prefix)
  __threadId: 'thread_789'               // ← middleware 주입 (__ prefix, 선택사항)
})
````

````

---

### 2. `playbook-store.ts` (Web MCP)

**위치**: `src/lib/web-mcp/modules/playbook-store.ts`

**현재 구조**:

```typescript
let currentAssistantId: string | null = null;  // ⚠️ 전역 변수

const playbookStore: WebMCPServer = {
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    case 'select_playbook': {
      // Permission check
      if (currentAssistantId && existing.agentId !== currentAssistantId) {
        return createMCPTextResponse(
          `[select_playbook] Error: Playbook ${id} does not belong to the current assistant (${currentAssistantId}).`
        );
      }
      // ...
    }
  },
  async switchContext(context: ServiceContextOptions): Promise<void> {
    const assistantId = context.assistantId;
    if (assistantId) {
      currentAssistantId = assistantId;  // ⚠️ 전역 상태 변경
    }
  },
};
````

**문제점**:

- `currentAssistantId` 전역 변수로 permission 검사
- 동시 요청 시 권한 검사가 잘못된 assistant에 대해 수행될 수 있음

**파라미터 전달 필요**:

```typescript
// Tool definition (AI가 보는 것 - 변경 없음)
{
  "name": "select_playbook",
  "inputSchema": {
    "type": "object",
    "properties": {
      "id": { "type": "string" }
      // ← assistantId 없음
    }
  }
}

// Tool invocation (backend에서 받는 것 - middleware 주입)
callTool('select_playbook', {
  id: 'pb_123',                       // AI가 생성
  __sessionId: 'sess_123',            // ← middleware 주입
  __assistantId: 'asst_456'           // ← middleware 주입 (권한 검사용)
})
```

---

### 3. `content_store` (Rust Backend)

**위치**: `src-tauri/src/mcp/builtin/content_store/server.rs`

**현재 구조**:

```rust
pub struct ContentStoreServer {
    pub(crate) session_manager: Arc<SessionManager>,  // ← 싱글톤
    pub(crate) storage: Mutex<ContentStoreStorage>,
}

pub async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String> {
    if let Some(session_id) = &options.session_id {
        // SessionManager에 전역 상태 설정
        self.session_manager.set_session_async(session_id.clone()).await?;

        // Storage에서 현재 session의 store를 생성/초기화
        let mut storage = self.storage.lock().await;
        storage.get_or_create_store(session_id.clone(), ...).await?;
    }
    Ok(())
}

// Tool handlers are session-aware but rely on switch_context
pub async fn handle_add_content(&self, args: Value) -> MCPResponse {
    // Must call switch_context first to set session in SessionManager
    let session_id = self.session_manager.get_current_session()
        .ok_or("No active session")?;  // ⚠️ switch_context에 의존
    // ...
}
```

**문제점**:

- `SessionManager`는 싱글톤 (앱 전체에 하나)
- `switch_context`가 `SessionManager.current_session`을 전역으로 설정
- 도구 호출 중 다른 thread에서 `switch_context`를 호출하면 race condition

**파라미터 전달 필요**:

```rust
// Tool definition (AI가 보는 것 - 변경 없음)
{
  "name": "add_content",
  "inputSchema": {
    "type": "object",
    "properties": {
      "content": { "type": "string" },
      "filename": { "type": "string" }
      // ← sessionId 없음
    }
  }
}

// Tool invocation (backend에서 받는 것 - middleware 주입)
{
  "content": "...",
  "filename": "test.txt",
  "__sessionId": "sess_123",    // ← middleware 주입
  "__assistantId": "asst_456"   // ← middleware 주입
}
```

---

### 4. `workspace` (Rust Backend)

**위치**: `src-tauri/src/mcp/builtin/workspace/mod.rs`

**현재 구조**:

```rust
pub struct WorkspaceServer {
    session_manager: Arc<SessionManager>,  // ← 싱글톤
    isolation_manager: SessionIsolationManager,
    process_registry: ProcessRegistry,
}

pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager
        .get_current_session()  // ⚠️ switch_context로 설정된 상태 사용
        .unwrap_or_else(|| "default".to_string());

    // Session access verification
    let registry = self.process_registry.read().await;
    match registry.entries.get(process_id) {
        Some(entry) if entry.session_id == session_id => { /* OK */ },
        _ => { /* Error */ }
    }
}
```

**문제점**:

- 마찬가지로 `session_manager.get_current_session()` 의존
- Process registry는 session별로 격리되지 않음 (session_id로만 필터링)

**파라미터 전달 필요**:

```rust
// Tool definition (AI가 보는 것 - 변경 없음)
{
  "name": "poll_process",
  "inputSchema": {
    "type": "object",
    "properties": {
      "processId": { "type": "string" }
      // ← sessionId 없음
    }
  }
}

// Tool invocation (backend에서 받는 것 - middleware 주입)
{
  "processId": "proc_123",
  "__sessionId": "sess_123"    // ← middleware 주입
}
```

---

## Refactoring Outline

### Critical Design Principle: Tool Definition Separation

⚠️ **Most Important**: `sessionId`, `assistantId`, `threadId` must NEVER appear in Tool Definition (JSON Schema)

```
┌─────────────────────────────────────────────────────────┐
│ AI (sees this)                                          │
├─────────────────────────────────────────────────────────┤
│ Tool Definition (JSON Schema)                           │
│  - name: "create_goal"                                  │
│  - inputSchema.properties: { "goal": string }           │
│  ✅ No sessionId, assistantId, threadId                 │
└──────────────────────┬──────────────────────────────────┘
                       │ (AI generates args)
                       │ { "goal": "Learn Rust" }
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Frontend Middleware (use-tool-processor.ts)             │
├─────────────────────────────────────────────────────────┤
│ Transform:                                              │
│  Input:  { "goal": "Learn Rust" }                       │
│  Output: {                                              │
│    "goal": "Learn Rust",                                │
│    "__sessionId": "sess_123",     ← injected            │
│    "__assistantId": "asst_456"    ← injected            │
│  }                                                      │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Backend (planning-server.ts, content_store/server.rs)   │
├─────────────────────────────────────────────────────────┤
│ 1. Extract context: __sessionId, __assistantId          │
│ 2. Remove __ prefix fields from args                    │
│ 3. Use extracted context for state isolation            │
│ 4. Process clean args { "goal": "Learn Rust" }          │
└─────────────────────────────────────────────────────────┘
```

**Key Rules**:

1. ✅ **DO**: Inject `__sessionId`, etc. in middleware layer
2. ✅ **DO**: Extract and use in backend tool handlers
3. ❌ **DON'T**: Include in Tool Definition (JSON schema)
4. ❌ **DON'T**: Let AI know about these fields
5. ❌ **DON'T**: Modify AI-generated args (only append \_\_ prefix fields)

---

### Phase 1: Type & Interface Changes

#### 1.1 Frontend: `ServiceContextOptions` 확장

**파일**: `src/features/tools/index.tsx`

**현재**:

```typescript
export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
}
```

**변경 후**:

```typescript
export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string; // 선택사항: agent flow 추적
}
```

✅ **주의**: 기존 코드와 호환 (모두 optional)

---

#### 1.2 Backend Rust: `ServiceContextOptions` 동기화

**파일**: `src-tauri/src/mcp/types.rs`

**변경 예시**:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContextOptions {
    pub session_id: Option<String>,
    pub assistant_id: Option<String>,  // ← 추가
    pub thread_id: Option<String>,      // ← 추가
}
```

✅ **주의**: Rust의 `session_id` (snake_case) ↔ TypeScript의 `sessionId` (camelCase) 매핑 필수

---

#### 1.3 Tool Args에 Context 주입 (Critical: AI에 노출되지 않음)

**⚠️ 중요**: sessionId, assistantId, threadId는 **Tool Definition (AI에 노출)**에 포함되면 안 됨!

- Tool schema는 기존 그대로 유지
- Context는 **frontend middleware**에서만 자동 주입
- Backend tool handler가 args에서 꺼내쓰기만 함

**전체 도구 호출의 공통 패턴**:

```typescript
// 현재 (useToolProcessor에서)
const response = await executeToolCall({
  id: toolCallId,
  type: 'function',
  function: {
    name: finalToolName,
    arguments: JSON.stringify(params), // ← AI가 생성한 args
  },
});

// 변경 후 (UseToolProcessor - middleware 레이어)
// AI가 생성한 params는 그대로 두고,
// 호출 직전에 middleware에서 context를 투명하게 주입
const paramsWithContext = {
  ...JSON.parse(toolCall.function.arguments), // AI 생성 args
  __sessionId: currentSession?.id, // ← middleware 주입 (비표준 필드)
  __assistantId: currentAssistant?.id, // ← middleware 주입 (비표준 필드)
  __threadId: someThreadId, // ← middleware 주입 (비표준 필드)
};

const response = await executeToolCall({
  id: toolCallId,
  type: 'function',
  function: {
    name: finalToolName,
    arguments: JSON.stringify(paramsWithContext),
  },
});
```

**설계 원칙**:

- **Tool Definition (MCPTool.inputSchema)**: 원래대로 유지 → AI만 본다
- **Tool Invocation (callTool 시점)**: `__` prefix로 context fields 주입 → backend만 사용
- **AI 관점**: 이들 필드는 "없는 것"처럼 동작
- **Backend 관점**: args에서 `__sessionId` 같은 필드를 꺼내 사용

**주의점**:

- `__` prefix 사용으로 AI가 실수로 사용할 가능성 최소화
- Tool schema에는 절대 추가 금지
- Backend에서만 내부적으로 처리

---

### Phase 2: Web MCP Servers 리팩토링

#### 2.1 `planning-server.ts`

**변경 사항**:

1. `switchContext` 메서드 **제거**
2. `SessionStateManager` → 파라미터 기반 context lookup
3. 모든 tool handler에서 context 파라미터 추출

**Before**:

```typescript
const planningServer: WebMCPServer = {
  async switchContext(context: ServiceContextOptions): Promise<void> {
    const sessionId = context.sessionId;
    if (sessionId) {
      stateManager.setSession(sessionId);
    }
  },
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    case 'create_goal': {
      return stateManager.createGoal(typedArgs.goal as string);
    }
  }
};
```

**After**:

```typescript
// 파라미터 기반 context 추출 함수 (backend helper)
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

// Map<sessionId::assistantId::threadId, EphemeralState>
const ephemeralStates = new Map<string, EphemeralState>();

function getStateKey(
  sessionId: string,
  assistantId?: string,
  threadId?: string
): string {
  return `${sessionId}::${assistantId || 'none'}::${threadId || 'default'}`;
}

const planningServer: WebMCPServer = {
  // switchContext 제거 ✅
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // Backend에서만 context 추출 (AI는 이 필드들을 모름)
    const context = extractContext(args as Record<string, unknown>);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId
    );

    let state = ephemeralStates.get(stateKey);
    if (!state) {
      state = new EphemeralState();
      ephemeralStates.set(stateKey, state);
    }

    // AI가 전달한 실제 args는 context 필드 제외
    const aiArgs = { ...args };
    delete aiArgs['__sessionId'];
    delete aiArgs['__assistantId'];
    delete aiArgs['__threadId'];

    case 'create_goal': {
      return state.createGoal((aiArgs as any).goal);
    }
  }
};
```

**주의**:

- 모든 tool handler에서 context 추출 필수
- State key 충돌 가능성 고려 (separator 명확히)
- Memory leak: 오래된 state 자동 정리 필요 (TTL 또는 explicit cleanup)

---

#### 2.2 `playbook-store.ts`

**변경 사항**:

1. `switchContext` 제거
2. `currentAssistantId` 전역 변수 제거
3. Tool args에서 `assistantId` 추출하여 permission 검사

**Before**:

```typescript
let currentAssistantId: string | null = null;

case 'select_playbook': {
  if (currentAssistantId && existing.agentId !== currentAssistantId) {
    return createMCPTextResponse(`Error: not allowed`);
  }
}
```

**After**:

```typescript
const playbookStore: WebMCPServer = {
  async callTool(
    name: string,
    args: unknown
  ): Promise<MCPResponse<unknown>> {
    // Backend에서만 context 추출
    const assistantId = (args as Record<string, unknown>).__assistantId as
      | string
      | undefined;

    case 'select_playbook': {
      // Permission check: backend에서만 검사
      if (
        assistantId &&
        existing.agentId !== assistantId
      ) {
        return createMCPTextResponse(
          `[select_playbook] Error: Playbook ${id} does not belong to assistant ${assistantId}.`
        );
      }
      // ...
    }
  },
  // switchContext 제거 ✅
};
```

---

### Phase 3: Rust Backend 리팩토링

#### 3.1 `content_store/server.rs`

**변경 사항**:

1. `switch_context` 제거
2. Tool handler들에서 args에서 `session_id`, `assistant_id` 추출
3. `SessionManager.get_current_session()` 대신 args 사용

**Before**:

```rust
pub async fn handle_add_content(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager
        .get_current_session()  // ⚠️ 전역 상태
        .ok_or("No active session")?;

    let mut storage = self.storage.lock().await;
    storage.add_content(session_id, ...)?;
}
```

**After**:

```rust
pub async fn handle_add_content(&self, args: Value) -> MCPResponse {
    // Backend에서만 context 추출
    let session_id: String = args
        .get("__sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    let mut storage = self.storage.lock().await;

    // session_id를 명시적으로 전달
    let store = storage.get_or_create_store(&session_id, ...)?;

    // AI가 전달한 실제 args에서 context 필드 제외
    let ai_args = remove_internal_fields(&args); // __sessionId, __assistantId 제거
    store.add_content(ai_args)?;
}
```

**주의**:

- `SessionManager` 의존성 제거 (또는 최소화)
- Content store에서 session별 isolation 명확히 (SQL query에 session_id 필터 필수)

---

#### 3.2 `workspace/mod.rs`

**변경 사항**:

1. `switch_context` 제거
2. Tool handler들에서 args에서 `session_id` 추출
3. Process registry 접근 시 session_id 필터 적용

**Before**:

```rust
pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let session_id = self.session_manager
        .get_current_session()  // ⚠️ 전역 상태
        .unwrap_or_else(|| "default".to_string());
}
```

**After**:

```rust
pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    // Backend에서만 context 추출
    let session_id: String = args
        .get("__sessionId")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    // session_id를 명시적으로 사용
    let registry = self.process_registry.read().await;
    if let Some(entry) = registry.entries.get(process_id) {
        if entry.session_id != session_id {
            return Self::error_response(
                request_id,
                -32603,
                "Access denied: process belongs to different session"
            );
        }
    }
}
```

---

### Phase 4: Frontend Hook 업데이트

#### 4.1 `use-tool-processor.ts`

**변경 사항**:

```typescript
export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  // ...

  const toolPromises = tcMessage.tool_calls
    .map(fixInvalidToolCall)
    .map(async (toolCall) => {
      // ⚠️ Critical: AI가 생성한 args를 건드리지 않고,
      // middleware에서 context fields만 추가
      const middlewareInjectedArgs = {
        ...JSON.parse(toolCall.function.arguments), // AI args 그대로
        __sessionId: currentSession?.id, // ← 중간 레이어 주입
        __assistantId: currentAssistant?.id, // ← 중간 레이어 주입
        __threadId: someThreadId, // ← 중간 레이어 주입 (선택)
      };

      const mcpResponse = await executeToolCallRef.current({
        ...toolCall,
        function: {
          ...toolCall.function,
          arguments: JSON.stringify(middlewareInjectedArgs),
        },
      });
    });
};
```

**설계 포인트**:

- AI가 생성한 args는 **절대 수정 금지**
- `__` prefix로 명확히 infrastructure fields 구분
- Backend에서 `__` prefix 필드를 꺼내 사용
- Tool definition에는 절대 반영 안 함

---

#### 4.2 `use-unified-mcp.ts`

**변경 사항**: 기존 로직 유지 (context 추가는 호출처에서)

---

### Phase 5: Backend MCP Trait 업데이트

#### 5.1 `BuiltinMCPServer` trait 수정

**파일**: `src-tauri/src/mcp/builtin/mod.rs`

**Before**:

```rust
#[async_trait]
pub trait BuiltinMCPServer {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;
    async fn switch_context(&self, options: ServiceContextOptions) -> Result<(), String>;
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

**After**:

```rust
#[async_trait]
pub trait BuiltinMCPServer {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;
    // ✅ switchContext 제거
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;
}
```

#### 5.2 Impl 생성 함수 수정

**모든 서버** (`ContentStoreServer::impl`, `WorkspaceServer::impl` 등):

```rust
#[async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    // switchContext 구현 제거
    // call_tool에서 args 내 context 파싱
}
```

---

## Migration Path & Compatibility

### Step 1: Backward Compatibility Layer (선택사항)

현재 코드의 급격한 변경을 피하기 위해 **과도기 지원**:

```typescript
// use-tool-processor.ts - 과도 로직
const argWithContext = {
  ...JSON.parse(toolCall.function.arguments),
  sessionId: currentSession?.id, // 명시적 추가
  assistantId: currentAssistant?.id,
};

// 도구가 여전히 switchContext를 호출하는 경우 대비
// (무시하도록 처리)
```

### Step 2: 단계별 도구 마이그레이션

1. `planning-server.ts` (Web MCP, 상대적 단순)
2. `playbook-store.ts` (Web MCP)
3. `content_store` (Rust Backend)
4. `workspace` (Rust Backend)
5. Other servers (browser, etc.)

### Step 3: 호환성 테스트

- 단일 세션, 단일 AI 요청 (현재 case) ✓
- 다중 세션, 순차 요청
- 다중 세션, 동시 요청 (목표 case)
- Context 누락 시 fallback (default session)

---

## Edge Cases & Considerations

### 1. Context 누락 처리

```typescript
// Tool args에 sessionId 없는 경우
const sessionId = args.sessionId || 'default';
```

**정책**:

- 기본값으로 'default' session 사용
- 로깅: "Tool call without explicit sessionId"

### 2. Memory 관리 (Web MCP)

```typescript
// planning-server의 ephemeralStates 메모리 누적
const ephemeralStates = new Map<string, EphemeralState>();

// 정리 정책 (선택):
// Option A: TTL 기반 (마지막 접근으로부터 X시간 후 제거)
// Option B: LRU cache (최대 N개만 유지)
// Option C: 명시적 cleanup signal (session 종료 시)
```

### 3. ThreadId 추가 고려사항

**Agent agentic flow 추적용**:

- 한 goal에서 여러 sub-task/workflow 실행 시 thread별 isolation
- 현재 MVP: 미지원 (sessionId + assistantId만으로 충분)
- 향후 multi-turn agent flow 확장 시 추가

### 4. Permission Model 변경

현재 (switch_context 기반):

```
switch_context(sessionId=sess_1, assistantId=asst_1)
→ 이후 모든 호출이 asst_1에서 실행되는 것으로 간주
```

변경 후 (파라미터 기반):

```
callTool('select_playbook', {
  id: 'pb_1',
  sessionId: 'sess_1',
  assistantId: 'asst_1'
})
→ 명시적으로 asst_1에서만 이 playbook 실행 가능
```

**이점**:

- 각 요청이 명시적인 권한 정보 전달
- 권한 검사 로직이 도구 내부에서 투명하게 수행

---

## Implementation Checklist

### Key Design Constraint

- ⚠️ **Tool Definition**: NO sessionId/assistantId/threadId in JSON schema
- ✅ **Middleware Injection**: Add `__sessionId`, `__assistantId` in use-tool-processor
- ✅ **Backend Extraction**: Extract and remove `__` prefix fields before processing

### Frontend Changes

- [ ] `use-tool-processor.ts`: Inject `__sessionId`, `__assistantId`, `__threadId` (\_\_ prefix)
- [ ] Verify Tool definitions remain unchanged (no context fields)
- [ ] Test: AI-generated args ≠ injected args (append only, never modify)

### Backend Types

- [ ] Rust `ServiceContextOptions` 타입 유지 (이미 존재, 변경 불필요)
- [ ] Tool args에서 `__` prefix 필드 추출 로직 추가
- [ ] Helper function: `extract_context(args: Value) -> (sessionId, assistantId, threadId)`

### Web MCP Servers

- [ ] `planning-server.ts`:
  - [ ] `switchContext` 메서드 제거
  - [ ] `extractContext()` helper 추가
  - [ ] 모든 tool handler에서 `__` prefix 필드 추출 및 제거
  - [ ] `ephemeralStates` Map 생성 (sessionId::assistantId::threadId by key)
  - [ ] 기존 `stateManager.getCurrentState()` → key 기반 state lookup으로 변경
- [ ] `playbook-store.ts`:
  - [ ] `switchContext` 메서드 제거
  - [ ] `currentAssistantId` 전역 변수 제거
  - [ ] `select_playbook` tool handler에서 `__assistantId` 추출하여 권한 검사

### Rust Builtin Servers

- [ ] `BuiltinMCPServer` trait:
  - [ ] `switch_context` 메서드 제거
- [ ] `content_store/server.rs`:
  - [ ] `switch_context` 구현 제거
  - [ ] `handle_*` tool handlers에서 `__sessionId` 추출
  - [ ] `remove_internal_fields()` helper (\_\_ prefix 제거)
  - [ ] Session-scoped storage 접근 (명시적 session_id 파라미터)
- [ ] `workspace/mod.rs`:
  - [ ] `switch_context` 호출 제거 (이미 없을 수 있음)
  - [ ] `handle_*` tool handlers에서 `__sessionId` 추출
  - [ ] Process registry 접근 시 session_id 필터 명시

### Testing

- [ ] **Single Session**: Tool calls with sessionId work correctly
- [ ] **Multi-Session Sequential**: Same tool in different sessions isolated
- [ ] **Multi-Session Concurrent**: Parallel requests to different sessions don't interfere
- [ ] **Context Missing**: Fallback to 'default' session, log warning
- [ ] **Tool Definition Unchanged**: Verify no `__` prefix in schema
- [ ] **Backend Extraction**: Verify `__` prefix fields correctly extracted and used

---

## Expected Outcomes

### 1. Thread-Safety 개선

- ❌ Before: 전역 상태로 인한 race condition
- ✅ After: 각 호출이 독립적 context 전달 → race condition 불가능

### 2. 확장성

- ❌ Before: 새 context 필드 추가 시 모든 서버의 switchContext 수정
- ✅ After: args에 자동 추가 → 도구는 필요한 필드만 사용

### 3. 디버깅

- ❌ Before: "어느 세션인지" 추적 어려움 (전역 상태)
- ✅ After: 각 tool call log에 sessionId, assistantId 명시

### 4. Multi-tenant/Multi-agent

- ❌ Before: 다중 AI 요청 시 context 혼동
- ✅ After: 각 요청이 독립적 context 관리

---

## Risk Assessment

| Risk                         | Probability | Impact | Mitigation              |
| ---------------------------- | ----------- | ------ | ----------------------- |
| Context 누락                 | Medium      | Low    | Default value + logging |
| Memory leak (Web MCP)        | Medium      | Medium | TTL/LRU cleanup         |
| Backward incompatibility     | Low         | Medium | Gradual migration       |
| Performance (args 크기 증가) | Low         | Low    | Minimal overhead        |
| Complex refactoring          | High        | Medium | Phased approach         |

---

## References

- [Planning Server Current Code](../../../src/lib/web-mcp/modules/planning-server.ts)
- [Playbook Store Current Code](../../../src/lib/web-mcp/modules/playbook-store.ts)
- [Content Store Current Code](../../../src-tauri/src/mcp/builtin/content_store/)
- [Workspace Current Code](../../../src-tauri/src/mcp/builtin/workspace/)
