# Refactoring Plan: Agent V2 Backend Implementation

**Date**: 2024-12-27 14:30  
**Target Branch**: dev/0.4.0  
**Scope**: Backend (Rust) Implementation - Tool Execution Loop, Error Handling, Testing  
**Excluded**: Frontend Dual-Track Integration (separate task)

---

## 1. 작업의 목적

Agent V2 Architecture의 **Backend 핵심 로직**을 완성하여 조건부 이벤트 기반 Tool Execution Cycle을 구현하고, 안정적인 Error Recovery 메커니즘을 추가합니다.

**🔑 중요:** "Tool Execution Loop"은 재귀 호출이 아니라 **event-driven cycle**입니다. 각 `handle_llm_response` 호출은 완전히 종료되고(return) 스택이 해제된 후, 이벤트 발생으로 다음 사이클을 트리거합니다. **Call stack accumulation 없음** - 각 사이클은 독립적인 함수 호출입니다.

**주요 목표:**

- ✅ Tool Execution Loop의 조건부 재귀 패턴 구현
- ✅ Robust Error Handling (Crash Recovery, LLM Retry, Tool Error Handling)
- ✅ E2E Testing Suite 구축
- ✅ AgentSessionManager의 Production-Ready 완성

**비포함 항목 (별도 작업):**

- Frontend Dual-Track Routing (Session model flag, ChatContainer router)
- Web MCP Migration (mcp-manager, assistant-manager)
- UI 통합 및 사용자 경험 개선

---

## 2. 현재 상태 / 문제점

### 2.1. Tool Execution Loop 미구현 (Critical)

**현재 상태:**

```rust
// src-tauri/src/agent/session_manager.rs:115-150
pub async fn handle_llm_response(
    &self,
    session_id: &str,
    message: Message,
) -> Result<(), String> {
    self.add_message_to_history(session_id, &message).await?;
    // ❌ 여기서 종료: Tool call 체크 없음
    // ❌ Tool 실행 없음
    // ❌ 재귀적 LLM 요청 없음
    Ok(())
}
```

**문제점:**

- LLM이 tool call을 반환해도 실행되지 않음
- 단일 턴 대화만 가능 (Multi-turn agentic workflow 불가능)
- V1 (TypeScript)의 `useEffect` + `processToolCalls` 패턴이 Rust에 구현되지 않음

**예상 동작 (V1 패턴):**

```typescript
// src/context/ChatContext.tsx:642-648
useEffect(() => {
  const lastMessage = messages[messages.length - 1];
  if (lastMessage) {
    processToolCalls(lastMessage); // ← 조건 체크 및 실행
  }
}, [messages, processToolCalls]);

// src/hooks/use-tool-processor.ts:356-378
const processToolCalls = (message: Message) => {
  if (message.tool_calls && message.tool_calls.length > 0) {
    execute(message); // → submitRef.current(toolResults) → 재귀
  }
};
```

### 2.2. Error Handling 누락 (Important)

**현재 상태:**

- Crash Recovery: 없음 (앱 재시작 시 BUSY 상태 세션 방치)
- LLM Retry: 없음 (네트워크 실패 시 workflow 중단)
- Tool Error Handling: 부분적 (Error result 생성하지만 workflow 계속 진행 메커니즘 없음)

**문제점:**

- 앱 충돌 후 세션이 zombie 상태로 남음
- 일시적 네트워크 오류로 전체 workflow 실패
- Tool 오류 발생 시 사용자에게 복구 기회 없음

### 2.3. Testing 부족 (Important)

**현재 상태:**

- Integration Tests: 80% (MCPServiceProxyManager, session isolation)
- Unit Tests: 40% (일부 built-in servers)
- E2E Tests: 0% (전체 workflow 테스트 없음)

**문제점:**

- Multi-turn conversation 동작 검증 불가
- Concurrent agent 동작 검증 불가
- Error recovery 시나리오 검증 불가

---

## 3. 관련 코드의 구조 및 동작 방식 Summary (Bird's Eye View)

### 3.1. 현재 Architecture (80% 완성)

```
┌─────────────────────────────────────────────────────────────────┐
│                      Frontend (TypeScript)                       │
├─────────────────────────────────────────────────────────────────┤
│  LLMServiceContext                                               │
│  - Listens: llm:completion-request event                        │
│  - Executes: LLM API call via useAIService                      │
│  - Returns: invoke('agent_handle_llm_response', message)        │
│                                                                   │
│  AgentSessionContext                                             │
│  - Listens: agent:event (status updates, message reload)        │
│  - Manages: UI state synchronization                            │
└─────────────────────────────────────────────────────────────────┘
                              ↕ IPC (Tauri Commands + Events)
┌─────────────────────────────────────────────────────────────────┐
│                       Backend (Rust)                             │
├─────────────────────────────────────────────────────────────────┤
│  AgentSessionManager (src-tauri/src/agent/session_manager.rs)   │
│  ├─ create_session()          ✅ Complete                        │
│  ├─ start_workflow()          ✅ Complete (emits LLM request)    │
│  ├─ handle_llm_response()     ⚠️ Incomplete (no tool loop)      │
│  ├─ terminate_workflow()      ✅ Complete (CancellationToken)    │
│  └─ get_session()             ✅ Complete                        │
│                                                                   │
│  MCPServiceProxyManager (src-tauri/src/mcp/service_proxy_manager.rs) │
│  ├─ create_proxy()            ✅ Complete (session isolation)    │
│  ├─ execute_tool()            ✅ Complete (routing logic)        │
│  ├─ destroy_proxy()           ✅ Complete (cleanup)              │
│  └─ Cleanup Task              ✅ Complete (10min idle timeout)   │
│                                                                   │
│  Built-in MCP Servers (src-tauri/src/mcp/builtin/*/mod.rs)      │
│  ├─ BootstrapServer           ✅ Complete (stateless)            │
│  ├─ KnowledgeServer           ✅ Complete (session-scoped)       │
│  ├─ PlanningServer            ✅ Complete (session-scoped)       │
│  ├─ PlaybookServer            ✅ Complete (session-scoped)       │
│  ├─ AssistantServer           🔴 30% (basic structure only)      │
│  ├─ ContentStoreServer        🔴 50% (needs session refactor)    │
│  └─ WorkspaceServer           🟡 80% (needs trait verification)  │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2. V1 Tool Execution Flow (TypeScript - Reference Implementation)

```typescript
┌─────────────────────────────────────────────────────────────┐
│ 1. Message Added to History (addMessage/addMessages)       │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. useEffect Detects Message Change                        │
│    - messages dependency array triggers                     │
│    - Gets last message from array                           │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. processToolCalls(message) - Condition Check             │
│    - message.role === 'assistant'?                          │
│    - message.tool_calls?.length > 0?                        │
│    - !message.isStreaming?                                  │
│    - !loading && !isPending?                                │
└───────────────────────┬─────────────────────────────────────┘
                        ↓ (if all conditions true)
┌─────────────────────────────────────────────────────────────┐
│ 4. execute(message) - Tool Execution                        │
│    - Loop through tool_calls                                │
│    - executeToolCallRef.current(toolCall)                   │
│    - Build tool result messages                             │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ 5. submitRef.current(toolResultMessages)                    │
│    - Add tool results to history                            │
│    - Trigger LLM with updated context                       │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
                   (Back to Step 1)
```

### 3.3. V2 Required Flow (Rust - Target Implementation)

```rust
┌─────────────────────────────────────────────────────────────┐
│ 1. TypeScript calls: agent_handle_llm_response(message)    │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. handle_llm_response() - Save & Condition Check          │
│    - Save assistant message to DB                           │
│    - Extract tool_calls from message                        │
│    - Check: tool_calls.is_empty()?                          │
└───────────────────────┬─────────────────────────────────────┘
                        ↓ (if not empty)
┌─────────────────────────────────────────────────────────────┐
│ 3. execute_tool_calls() - Tool Execution                    │
│    - Loop through tool_calls                                │
│    - proxy_manager.execute_tool(session_id, tool_call)      │
│    - Handle errors (create error tool results)              │
│    - Save tool results to DB                                │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
┌─────────────────────────────────────────────────────────────┐
│ 4. emit_llm_completion_request() - Re-trigger LLM          │
│    - Emit llm:completion-request event                      │
│    - Include updated message history (with tool results)    │
└───────────────────────┬─────────────────────────────────────┘
                        ↓
                   (TypeScript LLMServiceContext hears event)
                   (Calls LLM, returns via agent_handle_llm_response)
                   (Back to Step 1 - New Independent Call, Not Recursion)

┌─────────────────────────────────────────────────────────────┐
│ Termination Path (if tool_calls.is_empty())                │
│ - emit_event(AgentEvent::WorkflowComplete)                  │
│ - update_workflow_status(WorkflowStatus::Idle)              │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. 변경 이후의 상태 / 해결 판정 기준

### 4.1. Tool Execution Loop 완성 기준

**기능 요구사항:**

- ✅ LLM response에 tool call이 있으면 자동으로 실행
- ✅ Tool 실행 후 결과를 포함하여 LLM 재요청
- ✅ Tool call이 없는 response가 올 때까지 반복
- ✅ CancellationToken으로 중단 가능
- ✅ Tool 실행 오류 시 error tool result 생성하여 계속 진행

**검증 방법:**

```rust
#[tokio::test]
async fn test_multi_turn_tool_execution() {
    // Given: Session with KnowledgeServer enabled
    let session_id = manager.create_session(agent_config).await?;

    // When: User asks "Search for 'rust' and summarize"
    manager.send_message(session_id, user_msg).await?;

    // Then: Expect sequence
    // 1. LLM responds with tool_calls: [searchKnowledge("rust")]
    // 2. Tool executes, returns results
    // 3. LLM called again with tool results
    // 4. LLM responds with summary (no tool calls)
    // 5. Workflow completes

    assert_eq!(session.status, WorkflowStatus::Idle);
    let messages = get_messages(session_id).await?;
    assert!(messages.iter().any(|m| m.role == "tool"));
    assert!(messages.last().unwrap().content.contains("summary"));
}
```

### 4.2. Error Handling 완성 기준

**기능 요구사항:**

- ✅ 앱 재시작 시 BUSY 상태 세션을 PAUSED로 전환
- ✅ LLM 호출 실패 시 exponential backoff로 최대 3회 재시도
- ✅ Tool 실행 실패 시 error message를 LLM에 전달하여 복구 시도
- ✅ 치명적 오류 시 workflow 중단 및 사용자 알림

**검증 방법:**

```rust
#[tokio::test]
async fn test_crash_recovery() {
    // Given: Session in BUSY state
    let session = create_busy_session().await?;

    // When: App restarts (simulate by creating new manager)
    let new_manager = AgentSessionManager::new(db_pool).await?;
    new_manager.recover_sessions().await?;

    // Then: Session should be PAUSED
    let recovered = new_manager.get_session(session.id).await?;
    assert_eq!(recovered.status, WorkflowStatus::Paused);
}

#[tokio::test]
async fn test_llm_retry_on_network_error() {
    // Given: Mock LLM that fails 2 times then succeeds
    let mock_llm = MockLLM::new().fail_count(2);

    // When: Workflow starts
    manager.start_workflow(session_id, user_msg).await?;

    // Then: Should retry and eventually succeed
    assert_eq!(mock_llm.call_count(), 3);
    assert_eq!(session.status, WorkflowStatus::Idle);
}
```

### 4.3. Testing 완성 기준

**Test Coverage 목표:**

- ✅ E2E Tests: 5개 이상 (single-turn, multi-turn, concurrent, error scenarios)
- ✅ Unit Tests: 90%+ (AgentSessionManager, error handlers)
- ✅ Integration Tests: 95%+ (session isolation, proxy lifecycle)

**필수 E2E Scenarios:**

1. Single-turn conversation (user → LLM → response)
2. Multi-turn with tool execution (user → LLM+tool → LLM → response)
3. Concurrent agents (2 sessions running in parallel)
4. Error recovery (network failure, tool error, cancellation)
5. Long conversation (10+ turns, context management)

---

## 5. 수정이 필요한 코드 및 코드 스니핏

### 5.1. handle_llm_response() 수정 (Priority 1)

**파일:** `src-tauri/src/agent/session_manager.rs`

**Before:**

```rust
pub async fn handle_llm_response(
    &self,
    session_id: &str,
    message: Message,
) -> Result<(), String> {
    self.add_message_to_history(session_id, &message).await?;
    Ok(())
}
```

**After:**

```rust
pub async fn handle_llm_response(
    &self,
    session_id: &str,
    message: Message,
) -> Result<(), String> {
    // 1. Save assistant message
    self.add_message_to_history(session_id, &message).await?;

    // 2. Check for tool calls (Condition Check)
    let tool_calls = match &message.tool_calls {
        Some(calls) if !calls.is_empty() => calls,
        _ => {
            // No tool calls → workflow complete
            self.emit_event(
                session_id,
                AgentEvent::WorkflowComplete { final_message: message }
            )?;
            self.update_workflow_status(session_id, WorkflowStatus::Idle).await?;
            return Ok(());
        }
    };

    // 3. Execute tool calls
    let tool_results = self.execute_tool_calls(session_id, tool_calls).await?;

    // 4. Save tool results
    for result in tool_results {
        self.add_message_to_history(session_id, &result).await?;
    }

    // 5. Emit LLM request to trigger next cycle (Event-driven, not recursive)
    self.emit_llm_completion_request(session_id).await?;

    // Function ends here, stack freed
    // TypeScript will invoke this function again after LLM call
    Ok(())
}
```

### 5.2. execute_tool_calls() 구현 (New Method)

**파일:** `src-tauri/src/agent/session_manager.rs`

```rust
async fn execute_tool_calls(
    &self,
    session_id: &str,
    tool_calls: &[ToolCall],
) -> Result<Vec<Message>, String> {
    let mut tool_results = Vec::new();

    for tool_call in tool_calls {
        // Check cancellation before each tool
        if self.is_cancelled(session_id).await? {
            return Err("Workflow cancelled".to_string());
        }

        // Emit tool execution event (for UI progress)
        self.emit_event(
            session_id,
            AgentEvent::ToolExecutionStarted {
                tool_name: tool_call.function.name.clone(),
            }
        )?;

        // Execute tool via proxy manager
        let result = match self.proxy_manager
            .execute_tool(session_id, tool_call)
            .await
        {
            Ok(response) => {
                // Success: Create tool result message
                self.create_tool_result_message(tool_call, response)
            }
            Err(e) => {
                // Error: Create error tool result (continue workflow)
                logger::warn!(
                    "Tool execution failed: {} - {}",
                    tool_call.function.name,
                    e
                );
                self.create_error_tool_result(tool_call, &e)
            }
        };

        tool_results.push(result);

        // Emit completion event
        self.emit_event(
            session_id,
            AgentEvent::ToolExecutionCompleted {
                tool_name: tool_call.function.name.clone(),
            }
        )?;
    }

    Ok(tool_results)
}

fn create_tool_result_message(
    &self,
    tool_call: &ToolCall,
    response: MCPResponse,
) -> Message {
    Message {
        id: create_id(),
        role: "tool".to_string(),
        content: response.content,
        tool_call_id: Some(tool_call.id.clone()),
        // ... other fields
    }
}

fn create_error_tool_result(
    &self,
    tool_call: &ToolCall,
    error: &str,
) -> Message {
    Message {
        id: create_id(),
        role: "tool".to_string(),
        content: vec![MCPContent::Text {
            text: format!(
                "Error executing {}: {}",
                tool_call.function.name,
                error
            ),
        }],
        tool_call_id: Some(tool_call.id.clone()),
        error: Some(MessageError {
            display_message: format!("Tool execution failed: {}", error),
            error_type: "TOOL_EXECUTION_ERROR".to_string(),
            recoverable: true,
            // ... other fields
        }),
        // ... other fields
    }
}
```

### 5.3. Crash Recovery 구현 (New Method)

**파일:** `src-tauri/src/agent/session_manager.rs`

```rust
/// Called on app startup to recover sessions stuck in BUSY state
pub async fn recover_sessions(&self) -> Result<(), String> {
    let sessions = self.get_all_sessions().await?;

    for session in sessions {
        if session.status == WorkflowStatus::Busy {
            logger::info!(
                "Recovering session {} from BUSY state",
                session.id
            );

            // Reset to PAUSED (user can manually resume)
            self.update_workflow_status(
                &session.id,
                WorkflowStatus::Paused
            ).await?;

            // Emit recovery event
            self.emit_event(
                &session.id,
                AgentEvent::SessionRecovered {
                    previous_status: WorkflowStatus::Busy,
                }
            )?;
        }
    }

    Ok(())
}
```

**Integration Point:**

```rust
// src-tauri/src/lib.rs
#[tauri::command]
async fn initialize_agent_system(
    state: State<'_, AgentSessionManager>,
) -> Result<(), String> {
    // Recover sessions on app startup
    state.recover_sessions().await?;
    Ok(())
}
```

### 5.4. LLM Retry Logic 구현 (Modified Method)

**파일:** `src-tauri/src/agent/session_manager.rs`

```rust
async fn emit_llm_completion_request_with_retry(
    &self,
    session_id: &str,
) -> Result<(), String> {
    let max_retries = 3;
    let base_delay = Duration::from_secs(1);

    for attempt in 0..max_retries {
        match self.emit_llm_completion_request(session_id).await {
            Ok(_) => return Ok(()),
            Err(e) if attempt < max_retries - 1 => {
                // Exponential backoff
                let delay = base_delay * 2u32.pow(attempt);
                logger::warn!(
                    "LLM request failed (attempt {}/{}): {}. Retrying in {:?}",
                    attempt + 1,
                    max_retries,
                    e,
                    delay
                );

                tokio::time::sleep(delay).await;
            }
            Err(e) => {
                // All retries failed
                logger::error!("LLM request failed after {} attempts: {}", max_retries, e);

                // Pause workflow and notify user
                self.update_workflow_status(
                    session_id,
                    WorkflowStatus::Error
                ).await?;

                self.emit_event(
                    session_id,
                    AgentEvent::WorkflowError {
                        error: format!("LLM request failed: {}", e),
                    }
                )?;

                return Err(format!("LLM request failed after retries: {}", e));
            }
        }
    }

    Ok(())
}
```

---

## 6. 재사용 가능한 연관 코드

### 6.1. MCPServiceProxyManager (완성, 재사용)

**파일:** `src-tauri/src/mcp/service_proxy_manager.rs`

**주요 기능:**

- `create_proxy(session_id, tools)`: Session별 proxy 생성
- `execute_tool(session_id, tool_call)`: Tool 실행 (built-in + external routing)
- `destroy_proxy(session_id)`: Proxy 정리

**사용 예시:**

```rust
// In AgentSessionManager
let proxy_manager = self.proxy_manager.clone();
let result = proxy_manager
    .execute_tool(session_id, &tool_call)
    .await?;
```

### 6.2. Built-in MCP Servers (완성, 재사용)

**파일:** `src-tauri/src/mcp/builtin/*/mod.rs`

**완성된 서버:**

- `BootstrapServer`: Platform detection
- `KnowledgeServer`: Session-scoped knowledge base
- `PlanningServer`: Session-scoped planning tools
- `PlaybookServer`: Session-scoped playbook rendering

**BuiltinMCPServer Trait:**

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + Debug {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;
    async fn call_tool(&self, tool_name: &str, args: Value)
        -> Result<MCPResult, String>;
}
```

### 6.3. Message Repository (완성, 재사용)

**파일:** `src-tauri/src/repositories/message_repository.rs`

**주요 메서드:**

```rust
pub async fn create_message(&self, message: &Message) -> Result<Message, String>;
pub async fn get_messages_by_session(&self, session_id: &str) -> Result<Vec<Message>, String>;
pub async fn delete_message(&self, message_id: &str) -> Result<(), String>;
```

### 6.4. Event Emission (완성, 재사용)

**파일:** `src-tauri/src/agent/session_manager.rs`

```rust
fn emit_event(&self, session_id: &str, event: AgentEvent) -> Result<(), String> {
    let app_handle = self.app_handle.clone();
    app_handle.emit("agent:event", event)?;
    Ok(())
}

pub enum AgentEvent {
    StatusChanged { session_id: String, status: WorkflowStatus },
    MessageAdded { session_id: String, message_id: String },
    ToolExecutionStarted { tool_name: String },
    ToolExecutionCompleted { tool_name: String },
    WorkflowComplete { final_message: Message },
    WorkflowError { error: String },
    SessionRecovered { previous_status: WorkflowStatus },
}
```

---

## 7. Test Code 추가 및 수정 가이드

### 7.1. E2E Test Suite 구조

**파일:** `src-tauri/tests/agent_e2e_tests.rs` (New)

```rust
mod e2e {
    use super::*;

    /// Helper: Create test environment with mock LLM
    async fn setup_test_env() -> TestEnv {
        TestEnv {
            manager: AgentSessionManager::new(test_db_pool()).await?,
            mock_llm: MockLLMService::new(),
        }
    }

    #[tokio::test]
    async fn test_single_turn_conversation() {
        // Arrange
        let env = setup_test_env().await;
        let session_id = env.manager.create_session(test_agent()).await?;

        // Act: User sends "Hello"
        env.manager.start_workflow(session_id, "Hello").await?;

        // Mock LLM responds with no tool calls
        env.mock_llm.set_response(MockResponse {
            content: "Hi! How can I help?",
            tool_calls: vec![],
        });

        // Wait for workflow completion
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert
        let session = env.manager.get_session(session_id).await?;
        assert_eq!(session.status, WorkflowStatus::Idle);

        let messages = env.manager.get_messages(session_id).await?;
        assert_eq!(messages.len(), 2); // user + assistant
        assert_eq!(messages[1].content, "Hi! How can I help?");
    }

    #[tokio::test]
    async fn test_multi_turn_with_tool_execution() {
        let env = setup_test_env().await;
        let session_id = env.manager.create_session(
            test_agent_with_tools(vec!["knowledge"])
        ).await?;

        // Turn 1: User asks to search
        env.manager.start_workflow(session_id, "Search for 'rust'").await?;

        // Turn 1 Response: LLM responds with tool call
        env.mock_llm.set_response(MockResponse {
            content: "",
            tool_calls: vec![ToolCall {
                id: "call_1",
                function: Function {
                    name: "builtin_knowledge__searchKnowledge",
                    arguments: json!({"query": "rust"}),
                },
            }],
        });

        // Wait for tool execution
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Turn 2 Response: LLM responds with summary (no tool calls)
        env.mock_llm.set_response(MockResponse {
            content: "Found 3 articles about Rust programming language.",
            tool_calls: vec![],
        });

        // Wait for workflow completion
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Assert
        let messages = env.manager.get_messages(session_id).await?;
        assert_eq!(messages.len(), 4); // user + assistant1 + tool + assistant2
        assert!(messages[2].role == "tool");
        assert!(messages[3].content.contains("Found 3 articles"));
    }

    #[tokio::test]
    async fn test_concurrent_agents() {
        let env = setup_test_env().await;

        // Create two sessions
        let session_a = env.manager.create_session(test_agent()).await?;
        let session_b = env.manager.create_session(test_agent()).await?;

        // Start workflows concurrently
        let handle_a = tokio::spawn({
            let manager = env.manager.clone();
            async move {
                manager.start_workflow(session_a, "Task A").await
            }
        });

        let handle_b = tokio::spawn({
            let manager = env.manager.clone();
            async move {
                manager.start_workflow(session_b, "Task B").await
            }
        });

        // Both should complete without interference
        let (result_a, result_b) = tokio::join!(handle_a, handle_b);
        assert!(result_a.is_ok());
        assert!(result_b.is_ok());

        // Verify isolation
        let messages_a = env.manager.get_messages(session_a).await?;
        let messages_b = env.manager.get_messages(session_b).await?;
        assert!(messages_a[0].content.contains("Task A"));
        assert!(messages_b[0].content.contains("Task B"));
    }

    #[tokio::test]
    async fn test_tool_error_recovery() {
        let env = setup_test_env().await;
        let session_id = env.manager.create_session(test_agent()).await?;

        // Configure mock tool to fail
        env.mock_tool.set_error("Network timeout");

        env.manager.start_workflow(session_id, "Use failing tool").await?;

        // LLM responds with tool call
        env.mock_llm.set_response_with_tool_call("failingTool");
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Tool fails, creates error result
        let messages = env.manager.get_messages(session_id).await?;
        let tool_result = messages.iter().find(|m| m.role == "tool").unwrap();
        assert!(tool_result.error.is_some());

        // LLM should receive error and continue
        env.mock_llm.set_response(MockResponse {
            content: "I encountered an error, let me try another approach.",
            tool_calls: vec![],
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Workflow should complete despite tool error
        let session = env.manager.get_session(session_id).await?;
        assert_eq!(session.status, WorkflowStatus::Idle);
    }

    #[tokio::test]
    async fn test_cancellation() {
        let env = setup_test_env().await;
        let session_id = env.manager.create_session(test_agent()).await?;

        // Start long-running workflow
        env.manager.start_workflow(session_id, "Long task").await?;

        // Mock slow tool execution
        env.mock_tool.set_delay(Duration::from_secs(10));
        env.mock_llm.set_response_with_tool_call("slowTool");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Cancel while tool is executing
        env.manager.terminate_workflow(session_id).await?;

        // Should stop immediately
        let session = env.manager.get_session(session_id).await?;
        assert_eq!(session.status, WorkflowStatus::Stopped);
    }
}
```

### 7.2. Unit Test 가이드

**파일:** `src-tauri/src/agent/session_manager_tests.rs` (Expand existing)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_llm_response_with_tool_calls() {
        // Test: handle_llm_response processes tool calls
        let manager = create_test_manager().await;
        let session_id = create_test_session(&manager).await;

        let message = Message {
            role: "assistant",
            content: "",
            tool_calls: Some(vec![test_tool_call()]),
            // ...
        };

        manager.handle_llm_response(session_id, message).await?;

        // Should have executed tool and saved result
        let messages = manager.get_messages(session_id).await?;
        assert!(messages.iter().any(|m| m.role == "tool"));
    }

    #[tokio::test]
    async fn test_handle_llm_response_without_tool_calls() {
        // Test: handle_llm_response completes workflow
        let manager = create_test_manager().await;
        let session_id = create_test_session(&manager).await;

        let message = Message {
            role: "assistant",
            content: "Final answer",
            tool_calls: None,
            // ...
        };

        manager.handle_llm_response(session_id, message).await?;

        // Should be idle
        let session = manager.get_session(session_id).await?;
        assert_eq!(session.status, WorkflowStatus::Idle);
    }

    #[tokio::test]
    async fn test_crash_recovery() {
        // Test: recover_sessions resets BUSY to PAUSED
        let manager = create_test_manager().await;
        let session_id = create_test_session(&manager).await;

        // Simulate crash: set status to BUSY directly in DB
        manager.update_workflow_status(session_id, WorkflowStatus::Busy).await?;

        // Simulate restart
        let new_manager = AgentSessionManager::new(test_db_pool()).await?;
        new_manager.recover_sessions().await?;

        // Should be PAUSED
        let session = new_manager.get_session(session_id).await?;
        assert_eq!(session.status, WorkflowStatus::Paused);
    }
}
```

### 7.3. Integration Test 확장

**파일:** `src-tauri/tests/proxy_manager_integration_tests.rs` (Existing, expand)

```rust
#[tokio::test]
async fn test_tool_execution_in_workflow_context() {
    // Test: Proxy manager executes tools correctly in workflow
    let manager = setup_proxy_manager().await;
    let session_id = "test-session";

    // Create proxy with knowledge server
    manager.create_proxy(session_id, vec!["knowledge"]).await?;

    // Execute tool call
    let tool_call = ToolCall {
        id: "call_1",
        function: Function {
            name: "builtin_knowledge__saveKnowledge",
            arguments: json!({"title": "Test", "content": "Data"}),
        },
    };

    let result = manager.execute_tool(session_id, &tool_call).await?;

    // Verify result structure
    assert!(result.content.len() > 0);
    assert!(result.content[0].text.contains("saved"));

    // Verify session isolation: Create another session
    let session_b = "test-session-b";
    manager.create_proxy(session_b, vec!["knowledge"]).await?;

    // Session B should not see Session A's data
    let search_call = ToolCall {
        id: "call_2",
        function: Function {
            name: "builtin_knowledge__searchKnowledge",
            arguments: json!({"query": "Test"}),
        },
    };

    let search_result = manager.execute_tool(session_b, &search_call).await?;
    assert!(search_result.content[0].text.contains("0 results"));
}
```

---

## 8. 추가 분석 과제

### 8.1. LLM Retry Strategy Tuning

**분석 필요 사항:**

- Exponential backoff의 최적 파라미터 (base delay, max retries)
- 어떤 에러 코드에서 retry를 시도할지 (transient vs permanent errors)
- Circuit breaker 패턴 필요 여부 (연속 실패 시 일시적 중단)

**제안:**

- 초기 구현: Base delay 1s, max retries 3, exponential backoff
- 추후 개선: Provider별 rate limit 고려, 사용자 설정 추가

### 8.2. Tool Execution Timeout

**분석 필요 사항:**

- Tool 실행 timeout 기본값 설정 (예: 30초)
- Timeout 발생 시 처리 방식 (에러로 처리 vs 재시도)
- Long-running tool 지원 방안 (streaming results)

**제안:**

- 초기 구현: 30초 timeout, 에러로 처리
- 추후 개선: Tool별 custom timeout, progress streaming

### 8.3. Concurrent Session Limit

**분석 필요 사항:**

- 동시 실행 가능한 세션 수 제한 필요성
- 리소스 사용량 모니터링 방안
- Session queue vs rejection 전략

**제안:**

- 초기 구현: 제한 없음 (사용자 책임)
- 추후 개선: 설정 가능한 limit, queue 시스템

---

## 9. Clarification Q-List

### Q1. LLM Retry 전략

**질문:** LLM 호출 실패 시 자동 재시도를 어느 레벨에서 할 것인가?

**옵션:**

- **A:** Rust backend에서 자동 재시도 (투명하게 처리)
- **B:** TypeScript LLMServiceContext에서 재시도 (기존 AIService 로직 활용)
- **C:** 둘 다 구현 (Backend는 event emission 재시도, Frontend는 LLM call 재시도)

**현재 제안:** Option B (TypeScript에서 재시도)

- 기존 AIService 인프라 재사용
- LLM provider별 에러 처리 로직 이미 존재
- Rust는 재시도 없이 실패 시 event 재발송만

> 답변: Option B

### Q2. Tool Error Handling 정책

**질문:** Tool 실행 실패 시 workflow를 계속할 것인가 중단할 것인가?

**옵션:**

- **A:** 항상 계속 (error tool result를 LLM에 전달하여 복구 시도)
- **B:** Tool별 설정 (critical tool 실패 시 중단, 나머지는 계속)
- **C:** 연속 실패 횟수로 판단 (3회 연속 실패 시 중단)

**현재 제안:** Option A (항상 계속)

- LLM이 에러를 보고 alternative approach 시도
- 사용자가 수동으로 중단 가능 (CancellationToken)
- Circuit breaker는 Frontend (use-tool-processor)에서 이미 구현됨

> 답변: 항상 계속

### Q3. Session Recovery UI Flow

**질문:** 앱 재시작 후 PAUSED 세션을 어떻게 표시할 것인가?

**옵션:**

- **A:** 자동으로 세션 목록에 "Paused" 배지 표시, 사용자가 Resume 버튼 클릭
- **B:** 재시작 시 Modal로 복구 가능한 세션 목록 표시, 선택적 resume
- **C:** Background로 자동 resume (사용자 개입 없음)

**현재 제안:** Option A (Passive notification)

- Session list에서 상태 확인 가능
- 사용자 명시적 선택으로 resume
- Backend는 PAUSED 상태만 설정, UI는 별도 작업

> Option A

### Q4. E2E Test Mocking Strategy

**질문:** E2E 테스트에서 LLM API를 어떻게 mock할 것인가?

**옵션:**

- **A:** HTTP Mock Server (wiremock 등)
- **B:** Trait-based Mock (TestLLMService implements LLMService)
- **C:** Event interception (llm:completion-request 이벤트 가로채기)

**현재 제안:** Option C (Event interception)

- 실제 IPC 경로를 테스트 (realistic)
- Mock LLM service가 이벤트 listen하고 response 반환
- 기존 아키텍처 수정 최소화

### Q5. Built-in Tool Migration Priority

**질문:** AssistantServer, ContentStoreServer, WorkspaceServer 완성 우선순위는?

**옵션:**

- **A:** 모두 Phase 1에 포함 (Backend 완성을 최우선)
- **B:** AssistantServer만 Phase 1, 나머지 Phase 2
- **C:** 모두 Phase 2로 미루고 Tool Execution Loop에 집중

**현재 제안:** Option C (Phase 2로 미룸)

- Tool loop가 동작하면 기존 Web MCP tools로도 테스트 가능
- Backend 핵심 로직 완성 최우선
- 완성된 서버 (Knowledge, Planning, Playbook)로 충분히 검증 가능

---

## 10. 작업 우선순위 및 예상 시간

| Task                        | Priority        | Est. Days | Dependencies |
| --------------------------- | --------------- | --------- | ------------ |
| 1. handle_llm_response 수정 | 🔴 Critical     | 1         | None         |
| 2. execute_tool_calls 구현  | 🔴 Critical     | 1         | Task 1       |
| 3. Error tool result 생성   | 🔴 Critical     | 0.5       | Task 2       |
| 4. Crash recovery 구현      | 🟡 Important    | 0.5       | None         |
| 5. LLM retry logic          | 🟡 Important    | 0.5       | Task 1       |
| 6. E2E test suite           | 🔴 Critical     | 2         | Task 1-3     |
| 7. Unit tests 확장          | 🟡 Important    | 1         | Task 1-5     |
| 8. Integration tests 확장   | 🟢 Nice-to-have | 0.5       | Task 2       |

**Total:** 6-7 days (약 1.5주)

**Milestone 1** (Day 1-3): Tool Execution Loop 완성

- Tasks 1-3 완료
- 수동 테스트로 multi-turn conversation 검증

**Milestone 2** (Day 4-5): Error Handling & Recovery

- Tasks 4-5 완료
- Crash recovery 수동 검증

**Milestone 3** (Day 6-7): Testing & Validation

- Tasks 6-8 완료
- CI/CD 통합
- Documentation 업데이트

---

## 11. 참고 문서

- [elaborated_idea.md](../../elaborated_idea.md) - Architecture 전체 설계
- [agent-v2-integration-status.md](../analysis/agent-v2-integration-status.md) - 현재 상태 분석
- [chat-feature-architecture.md](../architecture/chat-feature-architecture.md) - V1 참조 구현
- [MCPServiceProxyManager Tests](../../src-tauri/tests/proxy_manager_integration_tests.rs) - Integration test 예시
