# Agentic Workflow 기능의 Backend 이관 검토

## 문제점

- 현재 구조에서 Agentic Workflow에 대한 관리를 React Frontend에서 처리하고 있으며 이는 다음과 같은 문제를 발생시킴
  - Frontend에서 세션 전환에 따라 Agentic Flow의 Interruption이 발생되고 이로 인해서 불완전한 상태가 발생함
  - Frontend 상태에 무관하게 Agentic Workflow가 지속될 수 있어야함

## 해결 방안

- Agentic Workflow를 처리하기 위한 Logic을 Rust Backend로 이관
- TS는 Web Native의 강점을 누릴 수 있는 의존성 Layer를 Rust에 제공
  - LLM Provider 통합
- Agentic Workflow는 UI 상태와 별도로 독립적인 상태와 실행 환경을 제공받음 (Rust)

## 기대 효과

- Agentic Workflow는 더이상 기존 Frontend 상태에 영향을 받지 않으며 Frontend는 단순히 이 Agentic Workflow와 상호작용 혹은 표시하기 위한 기능만 가지게됨
- 게다가 이러한 독립적인 Agentic Workflow 환경을 통해 손쉽게 Multi Agent / Background Agent를 지원할 수 있음

## UI/UX 기획

- 배경에서 실행 중인 Agentic Flow 목록을 UI로 표시하며 각 Flow의 상태 변화 Busy / Idle을 App 상단 Bar 알림 메뉴를 통해 확인할 수 있으며 클릭시 다시 해당 세션으로 돌아갈 수 있음.
- AppSidebar의 Session Item에도 현재 Background에서 Agentic Workflow의 실행 여부 상태를 (Green Dot / Yellow Dot으로 표시함)
- Agentic Flow가 실행 중인 Session으로 돌아가면 ChatInput에 cancel 버튼이 활성화된 상태로 되어야 하며 이를 통해서 사용자는 필요시 실행중인 Workflow를 중단할 수 있음
- Agentic Flow 실행이 완료된 Session으로 돌아가면 ChatInput에 send 버튼이 활성화된 상태로 되어있으며 ChatInput의 텍스트와 버튼을 통해 추가적인 요청을 할 수 있음

## Rust <-> React 통합

- TS -> Rust: Tauri Command를 이용
- Rust -> TS: [Tauri v2의 Emitter & Listener 활용](https://v2.tauri.app/ko/develop/calling-frontend/)

### 통합 시나리오별 Seq Diagram

#### Chat 시작

- 사용자가 StartChatView에서 Agent를 선택하면 해당 Agent와의 Session이 생성됨
- 이때 TS -> Rust로 createSession이 명령이 호출되며 이를 통해 Rust Backend는 해당 세션을 DB에 생성하고 간소화된 Session 객체를 Return하여 TS에 전달
- AgentSessionManager는 Session의 CRUD를 직접적으로 담당하고 Active Session의 Life Cycle을 관리함
- AgentSessionManager는 Agent가 작업을 수행하는데 필요한 환경을 MCPServiceProxyManager를 통해 활성화함 (외부 MCP Server 실행 혹은 연결, Program 자체적으로 내장된 )

- Seq. Diagram

  ```puml
  @startuml
  User -> StartChatView: start chat
  StartChatView -> useAgentSession: create(agent, llmConfig)
  useAgentSession -> AgentSessionManager: createSession(agent, llmConfig)
  AgentSessionManager -> Database: create new session
  Database --> AgentSessionManager: session: { id, ...}
  AgentSessionManager -> MCPServiceProxyManager: connectMCPService(session.id, agent.tools)
  MCPServiceProxyManager --> MCPServiceProxyManager: createMCPServiceProxy(sessionId, agent.tools)
  MCPServiceProxyManager -> MCPServiceProxy: start()
  MCPServiceProxy -> MCPServiceProxy: init process for stdio MCP servers
  MCPServiceProxy -> MCPServiceProxy: connect http streamable MCP servers
  MCPServiceProxy -> MCPServiceProxy: init built-in tools with given sessionId
  loop for tool in builtInTools
  MCPServiceProxy -> BuiltIn_N : connect(sessionId)
  ...
  end
  MCPServiceProxyManager --> AgentSessionManager: connection results
  alt connection ok
  AgentSessionManager --> useAgentSession: { id,... }
  useAgentSession --> StartChatView: session: { id, ...}
  StartChatView --> User: redirect to ./chat/
  else connection nok
  AgentSessionManager -> useAgentSession: tool connection error
  end
  @enduml
  ```

#### Resume Chat History

- 사용자가 기존 AgentSession으로 복귀할 때 AgentSessionManager는 기존 Session 정보를 Load하게됨
- 아울러 Agent 설정에 따라 MCPServiceProxyManager의 연결을 설정하고 사용자의 요청을 대기

- Seq. Diagram

  ```puml
  @startuml
  User -> SessionHistory: select session
  SessionHistory -> useAgentSession: resumeSession(sessionId)
  useAgentSession -> AgentSessionManager: resumeSession(sessionId)
  alt !isActiveSession(sessionId)
  AgentSessionManager -> Database: load session and messages
  Database --> AgentSessionManager: session {id, messages, agent }
  end
  AgentSessionManager -> MCPServiceProxyManager: connectMCPService(session.id, agent.tools, session.savedBuiltInContext)
  MCPServiceProxyManager --> MCPServiceProxyManager: createMCPServiceProxy(sessionId, agent.tools)
  MCPServiceProxyManager -> MCPServiceProxy: start()
  MCPServiceProxy -> MCPServiceProxy: init process for stdio MCP servers
  MCPServiceProxy -> MCPServiceProxy: connect http streamable MCP servers
  MCPServiceProxy -> MCPServiceProxy: init built-in tools with given sessionId
  loop for tool in builtInTools
  MCPServiceProxy -> BuiltIn_N : connect(sessionId, savedBuiltInContext)
  ...
  end
  MCPServiceProxyManager --> AgentSessionManager: connection results
  alt connection ok
  AgentSessionManager --> useAgentSession: { id,... }
  useAgentSession --> StartChatView: session: { id, ...}
  StartChatView --> User: redirect to ./chat/
  else connection nok
  AgentSessionManager -> useAgentSession: tool connection error
  end
  @enduml
  ```

#### 요청 전달 및 Agentic Flow 시작

- 사용자가 ChatInput에 Message를 작성하여 Send Button을 누르면 trigger 됨
- 해당 Message는 Rust & TS 호환의 Concrete Message Type의 객체로 Rust에 전달됨
- Rust에서 전체 Message Stack으로 구성하여 다시 useAIService에 요청을

- Seq. Diagram

  ```puml
  @startuml
  User -> ChatInput: click send button
  ChatInput -> useAgentSession: sendRequest(userMessage)
  useAgentSession -> AgentSessionManager: pushMessage(userMessage)
  AgentSessionManager -> Database: upsert(userMessage)
  AgentSessionManager -> MCPServiceProxyManager: getServiceContext(sessionId)
  AgentSessionManager -> useAgentSession: submit(messages, llmConfig)
  useAgentSession --> useAgentSession: updateMessage(messages)
  useAgentSession -> ChatMessages: streaming content
  ChatMessages -> User: show streaming message
  ...
  useAgentSession -> AgentSessionManager: pushMessage(newMessage)
  AgentSessionManager -> Database: upsert(newMessage)
  alt hasToolCall(newMessage) && !hasUIResource(newMessage)
  note right: UI Resource 감지 시 자동 멈춤 (사용자 상호작용 대기)
  AgentSessionManager --> AgentSessionManager: extractToolCalls(newMessage)
  loop toolCalls
  AgentSessionManager -> MCPServiceProxyManager: call_tool()
  note right: whether external MCP or buildin tool API
  MCPServiceProxyManager -> MCPServiceProxy: call_tool()
  MCPServiceProxyManager --> AgentSessionManager: toolResult
  AgentSessionManager --> AgentSessionManager: pushMessage(convertMessage(toolResult))
  alt hasUIResource(toolResult)
  note right: 🔍 Tool Result에 UI Resource 포함 (mimeType: "text/html")
  AgentSessionManager --> AgentSessionManager: skip re-submit (자동 멈춤)
  note right: ⏸️ 사용자가 UI에서 버튼 클릭 등 상호작용 대기
  User -> UI: click button in UI Resource
  UI -> AgentSessionManager: handleUIAction → executeToolCall
  note right: UI Action이 새 Tool Result 생성 (UI Resource 없음)
  AgentSessionManager -> useAgentSession: submit(messages, llmConfig)
  note right: ▶️ 조건 충족 (hasToolCall && !hasUIResource) → 자동 재개
  end
  end
  note right: Tool Call 완료 후 조건부 re-submit
  end
  AgentSessionManager -> useAgentSession: submit(messages, llmConfig)
  useAgentSession --> useAgentSession: updateMessage(messages)
  ...
  @enduml
  ```

### UI Resource 기반 자동 멈춤/재개 메커니즘

#### 핵심 원리

**조건부 Re-Submit 로직:**

```
if (hasToolCall(lastMessage) && !hasUIResource(lastMessage)) {
    request_llm_completion(); // Workflow 계속
} else {
    // Workflow 자연스럽게 멈춤 (사용자 상호작용 대기)
}
```

#### Auto-Pause (자동 멈춤)

- **감지 조건**: Tool Result 메시지가 UI Resource를 포함
  - `MCPContent::Resource` 타입
  - `mimeType: "text/html"` 속성
- **동작**: `!hasUIResource()` = false → re-submit 건너뜀
- **상태**: Session은 `Busy` 상태 유지 (조건부 대기)
- **UI**: MessageRenderer가 UIResourceRenderer를 통해 HTML 렌더링

#### Auto-Resume (자동 재개)

- **트리거**: 사용자가 UI Resource와 상호작용 (버튼 클릭 등)
- **흐름**:
  1. `UIResourceRenderer` → `postMessage` 이벤트
  2. `handleUIAction` → `useUnifiedMCP.executeToolCall()`
  3. Tool 실행 → Tool Result 메시지 추가 (UI Resource 없음)
  4. `hasToolCall(lastMsg) && !hasUIResource(lastMsg)` = true
  5. 자동으로 `request_llm_completion()` 호출
- **결과**: Workflow 자연스럽게 재개

#### 장점

- ✅ **단순성**: 별도의 IPC 명령 불필요 (`agent_resume_workflow` 없음)
- ✅ **자연스러움**: UI Action 자체가 resume 트리거
- ✅ **상태 관리 간소화**: `Paused` 상태 불필요
- ✅ **다중 UI Resource 지원**: 연속된 UI Resource도 자동 처리
- ✅ **에러 처리 일관성**: Tool 실행 실패도 동일한 조건 로직으로 처리

#### 구현 예시

**Rust (AgentSessionManager)**:

```rust
// Tool Result 저장 후 조건 체크
let last_message = self.get_last_message(&session_id).await?;

if has_tool_calls(&last_message) && !has_ui_resource(&last_message) {
    // Workflow 계속 - LLM에 Tool Result 전달
    self.request_llm_completion(session_id).await?;
} else if has_ui_resource(&last_message) {
    // 자동 멈춤 - 사용자 상호작용 대기
    // (아무 작업 안 함 - 조건부 idle)
} else {
    // Tool Call 없음 - Workflow 완료
    self.update_session_status(&session_id, SessionStatus::Idle).await?;
}
```

**Helper Function**:

```rust
fn has_ui_resource(message: &Message) -> bool {
    message.content.iter().any(|content| {
        matches!(content, MCPContent::Resource { resource })
            && resource.get("mimeType")
                .and_then(|v| v.as_str())
                .map_or(false, |mime| mime == "text/html")
    })
}
```

### 요구사항

- 외부에서 Agent 정의 / 환경 정의 / Data 통합을 Runtime에 주입할 수 있어야 함
  - Agent 정의
    - name
    - system prompt
    - tools
  - Environment
    - Agent에 AI 기능을 공급할 AI Model API => Completion service
    - Agent의 도구 호출을 실제 실행할 환경을 제공 => Tool service
    - Agent의 응답 생성 및 Message History를 저장하고 관리하기 위한 Data Backend => Data service
    - 응답의 출력을 전달할 Output endpoint
- IPC / ITC를 통해 Frontend에서 쉽게 Agentic Workflow를 통합할 수 있어야 함
  - 가령 React Frontend에서 동적으로 위 요소들을 주입하고 Agentic Workflow를 실행할 수 있어야 함
  - 동시에 필요한 경우 해당 Agentic Flow의 응답을 실시간으로 전달받고 이를 Frontend에서 표시할 수 있어야함
