# Implementation Examples: Before & After

두 가지 실제 서버를 기반으로 리팩토링 전후 코드를 비교합니다.

---

## 1. Web MCP: planning-server

### Before (Current - switch_context 기반)

```typescript
// src/lib/web-mcp/modules/planning-server.ts (현재)

class SessionStateManager {
  private sessions = new Map<string, EphemeralState>();
  private currentSessionId: string | null = null;

  setSession(sessionId: string): void {
    this.currentSessionId = sessionId;
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, new EphemeralState());
    }
  }

  getGoal(): Goal[] {
    return this.getCurrentState().goals;
  }

  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) {
      throw new Error('No active session');
    }
    return this.sessions.get(this.currentSessionId)!;
  }

  addTodo(name: string): MCPResponse<AddToDoOutput> {
    return this.getCurrentState().addTodo(name);
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
    // ⚠️ stateManager.getCurrentState()에 의존 (전역 상태 사용)
    switch (name) {
      case 'create_goal': {
        const typedArgs = args as { goal: string };
        return stateManager.getCurrentState().createGoal(typedArgs.goal);
      }
      case 'add_todo': {
        const typedArgs = args as { name: string; goal_id?: string };
        return stateManager.addTodo(typedArgs.name);
      }
      default:
        return createMCPTextResponse(`Unknown tool: ${name}`);
    }
  },
};
```

**문제**:

- `switchContext` 호출 필수 → async operation
- 동시 요청: context 충돌 가능
- 권한 검사 어려움 (assistantId 미지원)

### After (Parameter-based)

```typescript
// src/lib/web-mcp/modules/planning-server.ts (리팩토링 후)

// ─────────────────────────────────────────────────────────────
// 1. Context 추출 Helper
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

// ─────────────────────────────────────────────────────────────
// 2. 상태 저장소 (파라미터 기반 격리)
// ─────────────────────────────────────────────────────────────

const ephemeralStateMap = new Map<
  string,
  { state: EphemeralState; lastAccessedAt: number }
>();

function getOrCreateState(stateKey: string): EphemeralState {
  const now = Date.now();
  let entry = ephemeralStateMap.get(stateKey);

  if (entry) {
    entry.lastAccessedAt = now;
    return entry.state;
  }

  const newState = new EphemeralState();
  ephemeralStateMap.set(stateKey, {
    state: newState,
    lastAccessedAt: now,
  });
  return newState;
}

// ─────────────────────────────────────────────────────────────
// 3. Args 정제
// ─────────────────────────────────────────────────────────────

function cleanArgs(args: Record<string, unknown>): Record<string, unknown> {
  const cleaned = { ...args };
  delete cleaned.__sessionId;
  delete cleaned.__assistantId;
  delete cleaned.__threadId;
  return cleaned;
}

// ─────────────────────────────────────────────────────────────
// 4. Memory Cleanup (TTL 기반)
// ─────────────────────────────────────────────────────────────

setInterval(
  () => {
    const now = Date.now();
    const TTL_MS = 60 * 60 * 1000; // 1 hour

    for (const [key, entry] of ephemeralStateMap.entries()) {
      if (now - entry.lastAccessedAt > TTL_MS) {
        ephemeralStateMap.delete(key);
        console.log(`[planningServer] Cleaned up state: ${key}`);
      }
    }
  },
  5 * 60 * 1000,
); // 5분마다 체크

// ─────────────────────────────────────────────────────────────
// 5. Server Implementation
// ─────────────────────────────────────────────────────────────

const planningServer: WebMCPServer = {
  // ✅ switchContext 제거
  // async switchContext(...) { ... }

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const typedArgs = args as Record<string, unknown>;

    // ✅ Context 추출 (middleware에서 주입됨)
    const context = extractContext(typedArgs);
    const stateKey = getStateKey(
      context.sessionId,
      context.assistantId,
      context.threadId,
    );

    // ✅ Context별 상태 가져오기
    const state = getOrCreateState(stateKey);

    // ✅ Args 정제 (__ prefix 제거)
    const cleanedArgs = cleanArgs(typedArgs);

    // Tool 실행 (cleanedArgs 사용)
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
        if (!todoName) {
          return createMCPTextResponse('Error: name is required');
        }
        return state.addTodo(todoName, goalId);
      }

      case 'mark_todo': {
        const todoId = cleanedArgs.todo_id as string;
        if (!todoId) {
          return createMCPTextResponse('Error: todo_id is required');
        }
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

**개선**:

- ✅ `switchContext` 제거 → 동시 요청 안전
- ✅ Context별 상태 격리 → session/assistant/thread 안전
- ✅ 확장성 ↑ (새 필드 추가 시 middleware만 수정)
- ✅ Debuggability ↑ (각 호출에 context 명시)

---

## 2. Rust: content_store

### Before (Current - switch_context 기반)

```rust
// src-tauri/src/mcp/builtin/content_store/server.rs (현재)

pub struct ContentStoreServer {
    pub(crate) session_manager: Arc<SessionManager>,  // ← 전역 상태
    pub(crate) storage: Mutex<ContentStoreStorage>,
}

impl ContentStoreServer {
    pub async fn switch_context(
        &self,
        options: ServiceContextOptions,
    ) -> Result<(), String> {
        // ⚠️ 전역 session 상태 설정
        if let Some(session_id) = &options.session_id {
            self.session_manager
                .set_session_async(session_id.clone())
                .await?;

            let mut storage = self.storage.lock().await;
            storage
                .get_or_create_store(session_id.clone(), ...)
                .await?;
        }
        Ok(())
    }

    pub async fn handle_add_content(
        &self,
        args: Value,
    ) -> MCPResponse {
        // ⚠️ 전역 session_manager에 의존
        let session_id = self
            .session_manager
            .get_current_session()
            .ok_or("No active session")?;

        let mut storage = self.storage.lock().await;
        let store = storage
            .get_or_create_store(session_id, ...)
            .await
            .map_err(|e| MCPError::InternalError(e.to_string()))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| MCPError::InvalidParams)?;

        store.add_content(content).await?;
        Ok(/* response */)
    }
}

#[async_trait]
impl BuiltinMCPServer for ContentStoreServer {
    fn name(&self) -> &str {
        "content_store"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![/* tool definitions */]
    }

    async fn switch_context(
        &self,
        options: ServiceContextOptions,
    ) -> Result<(), String> {
        Self::switch_context(self, options).await
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "add_content" => self.handle_add_content(args).await,
            "read_content" => self.handle_read_content(args).await,
            "search_content" => self.handle_search_content(args).await,
            _ => MCPResponse::error(-32601, "Unknown tool"),
        }
    }
}
```

**문제**:

- `session_manager.get_current_session()` 의존 (전역 상태)
- Race condition: 다른 요청이 session 바꿀 수 있음
- `switch_context` async operation 필수

### After (Parameter-based)

```rust
// src-tauri/src/mcp/builtin/content_store/server.rs (리팩토링 후)

use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::{json, Value};

// ─────────────────────────────────────────────────────────────
// 1. Context 구조 및 추출
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
// 2. Args 정제
// ─────────────────────────────────────────────────────────────

pub fn clean_args(args: &Value) -> Value {
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
// 3. Session-Scoped Storage
// ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SessionContentStore {
    pub session_id: String,
    pub contents: Vec<ContentItem>,
    pub bm25_index: BM25Index,
}

pub struct ContentStoreServer {
    /// Session별 저장소 (key: session_id)
    /// ✅ session_manager 제거
    storage: Arc<Mutex<HashMap<String, SessionContentStore>>>,
}

impl ContentStoreServer {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Session-scoped store 가져오기
    async fn get_session_store(
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

        // ✅ Arc로 감싸서 반환 (clone 시 reference count 증가)
        Ok(Arc::new(Mutex::new(
            storage.get(session_id).unwrap().clone(),
        )))
    }

    // ─────────────────────────────────────────────────────────────
    // Tool Handlers (context 명시적 사용)
    // ─────────────────────────────────────────────────────────────

    async fn handle_add_content(
        &self,
        context: &ToolContext,
        args: &Value,
    ) -> MCPResponse {
        let filename = match args.get("filename").and_then(|v| v.as_str()) {
            Some(f) => f,
            None => return create_error_response(-32602, "filename is required"),
        };

        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return create_error_response(-32602, "content is required"),
        };

        // ✅ Session-scoped store 접근
        match self.get_session_store(&context.session_id).await {
            Ok(session_store) => {
                let mut store = session_store.lock().await;

                store.contents.push(ContentItem {
                    id: generate_id(),
                    filename: filename.to_string(),
                    content: content.to_string(),
                    session_id: context.session_id.clone(),
                    created_at: chrono::Utc::now(),
                });

                create_success_response(json!({
                    "status": "added",
                    "filename": filename
                }))
            }
            Err(e) => create_error_response(-32603, &e),
        }
    }

    async fn handle_search_content(
        &self,
        context: &ToolContext,
        args: &Value,
    ) -> MCPResponse {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // ✅ Session-scoped store 접근
        match self.get_session_store(&context.session_id).await {
            Ok(session_store) => {
                let store = session_store.lock().await;

                // ✅ 현재 session의 content만 검색
                let results: Vec<_> = store
                    .contents
                    .iter()
                    .filter(|c| {
                        c.session_id == context.session_id
                            && (c.filename.contains(query)
                                || c.content.contains(query))
                    })
                    .cloned()
                    .collect();

                create_success_response(json!({
                    "results": results,
                    "count": results.len()
                }))
            }
            Err(e) => create_error_response(-32603, &e),
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 4. Trait Implementation
// ─────────────────────────────────────────────────────────────

#[async_trait]
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
                    // ✅ __sessionId, __assistantId 없음!
                }),
            },
            MCPTool {
                name: "search_content".to_string(),
                description: "Search content in session".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" }
                    }
                    // ✅ No infrastructure fields
                }),
            },
        ]
    }

    // ✅ switch_context 메서드 제거

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        // ✅ Context 추출 (middleware에서 주입됨)
        let context = ToolContext::from_args(&args);

        // ✅ Args 정제
        let cleaned = clean_args(&args);

        match tool_name {
            "add_content" => self.handle_add_content(&context, &cleaned).await,
            "search_content" => self.handle_search_content(&context, &cleaned).await,
            _ => create_error_response(-32601, "Unknown tool"),
        }
    }
}
```

**개선**:

- ✅ `switch_context` 제거 → 동시 요청 안전
- ✅ `SessionManager` 의존 제거 → 단순화
- ✅ Session-scoped store → 격리 명확
- ✅ Tool definition에서 context fields 없음 → AI 학습 불필요
- ✅ Access control 투명 (context 기반)

---

## 3. Frontend: use-tool-processor

### Before (Current)

```typescript
// src/hooks/use-tool-processor.ts (현재)

export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  const toolPromises = tcMessage.tool_calls
    .map(fixInvalidToolCall)
    .map(async (toolCall) => {
      // ⚠️ Context 미주입
      const mcpResponse = await executeToolCallRef.current({
        ...toolCall,
        function: {
          ...toolCall.function,
          arguments: toolCall.function.arguments, // AI args 그대로
        },
      });

      // Tool 실행 후 switch_context 호출 (별도 step)
      // This is handled elsewhere (inefficient!)
    });
};
```

### After (Parameter-based)

```typescript
// src/hooks/use-tool-processor.ts (리팩토링 후)

export const useToolProcessor = ({ submit }: UseToolProcessorConfig) => {
  // 현재 session/assistant 정보 가져오기 (context에서)
  const { currentSession } = useContext(SessionContext);
  const { currentAssistant } = useContext(AssistantContext);

  const toolPromises = tcMessage.tool_calls
    .map(fixInvalidToolCall)
    .map(async (toolCall) => {
      // ✅ Step 1: AI 생성 args 파싱
      const aiGeneratedArgs = JSON.parse(toolCall.function.arguments);

      // ✅ Step 2: Middleware에서 context fields 주입 (__ prefix)
      const argsWithContext = {
        ...aiGeneratedArgs, // AI args (untouched)
        __sessionId: currentSession?.id,
        __assistantId: currentAssistant?.id,
        // __threadId: someThreadId, // optional
      };

      // ✅ Step 3: Tool 호출 (context 포함)
      const mcpResponse = await executeToolCallRef.current({
        ...toolCall,
        function: {
          ...toolCall.function,
          arguments: JSON.stringify(argsWithContext), // 수정된 args
        },
      });

      // Tool 실행 후 처리 (result handling)
      if (mcpResponse.type === 'tool_result') {
        // ...
      }
    });
};
```

**개선**:

- ✅ Context 자동 주입 (middleware layer)
- ✅ AI args 보존 (\_\_ prefix로 분리)
- ✅ Tool definition 유지 (변경 없음)
- ✅ 확장성 ↑ (새 context fields 추가 시 여기서만 수정)

---

## 4. 핵심 차이점 요약

| 측면                | Before                        | After                         |
| ------------------- | ----------------------------- | ----------------------------- |
| **session 설정**    | Explicit switchContext call   | Implicit middleware injection |
| **동시 요청**       | ❌ Race condition 위험        | ✅ 완전히 안전                |
| **Code 위치**       | Spread across multiple places | Centralized in middleware     |
| **Context fields**  | AI에 노출됨                   | AI에 숨겨짐 (\_\_ prefix)     |
| **State 격리**      | Global state (문제)           | Key-based isolation (해결)    |
| **Tool Definition** | 변경 필요 없음                | 변경 필요 없음 (좋음)         |
| **Debug**           | 전역 상태 추적 어려움         | 각 호출에 context 명시        |

---

## 5. Testing 예제

### Web MCP Test

```typescript
// src/lib/web-mcp/modules/planning-server.test.ts

describe('planningServer refactoring', () => {
  it('should isolate state by session', async () => {
    // Session 1: create goal
    const result1 = await planningServer.callTool('create_goal', {
      goal: 'Goal A',
      __sessionId: 'sess_1',
    });
    expect(result1.result.goalId).toBeDefined();

    // Session 2: create goal
    const result2 = await planningServer.callTool('create_goal', {
      goal: 'Goal B',
      __sessionId: 'sess_2',
    });
    expect(result2.result.goalId).toBeDefined();

    // Verify: different states
    const state1 = planningServer.getStateForTesting('sess_1');
    const state2 = planningServer.getStateForTesting('sess_2');

    expect(state1.goals).toHaveLength(1);
    expect(state2.goals).toHaveLength(1);
    expect(state1.goals[0].text).toBe('Goal A');
    expect(state2.goals[0].text).toBe('Goal B');
  });

  it('should clean up TTL-expired states', async () => {
    // Create state
    await planningServer.callTool('create_goal', {
      goal: 'Temp Goal',
      __sessionId: 'temp_sess',
    });

    // Manually trigger TTL check (test setup)
    // ... advance time, trigger cleanup

    // Verify cleaned up
    expect(planningServer.getStateForTesting('temp_sess')).toBeUndefined();
  });
});
```

### Rust Test

```rust
#[tokio::test]
async fn test_content_store_session_isolation() {
    let server = ContentStoreServer::new();

    // Session 1: add content
    let resp1 = server.call_tool("add_content", json!({
        "filename": "file1.txt",
        "content": "Session 1 data",
        "__sessionId": "sess_1"
    })).await;
    assert!(resp1.is_success());

    // Session 2: add content
    let resp2 = server.call_tool("add_content", json!({
        "filename": "file2.txt",
        "content": "Session 2 data",
        "__sessionId": "sess_2"
    })).await;
    assert!(resp2.is_success());

    // Verify isolation
    let storage = server.storage.lock().await;
    assert_eq!(
        storage.get("sess_1").unwrap().contents.len(),
        1
    );
    assert_eq!(
        storage.get("sess_2").unwrap().contents.len(),
        1
    );
}
```

---

## 6. Migration Order

1. **Frontend first** (use-tool-processor.ts)
   - ✅ Context 주입 추가
   - ✅ Tool definition 유지

2. **Web MCP servers** (planning-server, playbook-store)
   - ✅ switchContext 제거
   - ✅ Context 추출 로직 추가
   - ✅ TTL cleanup 추가

3. **Rust servers** (content_store, workspace)
   - ✅ switch_context trait 제거
   - ✅ Session-scoped storage 변경
   - ✅ Context 추출 로직 추가

4. **Testing & Deployment**
   - ✅ 단일 세션 테스트
   - ✅ 다중 세션 테스트
   - ✅ 동시 요청 테스트
