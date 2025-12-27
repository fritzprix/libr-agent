# Agent V2 Message State Synchronization Fix

**Date:** 2024-12-27
**Task ID:** `refactoring_20241227_message_state_sync`
**Related Documents:**

- [idea.md](../../idea.md) - Section: "요청 전달 및 Agentic Flow 시작" (Line: `useAgentSession --> useAgentSession: updateMessage(messages)`)
- [elaborated_idea.md](../../elaborated_idea.md) - Section 3.3 "LLM Integration", Section 4.2 "Starting a Workflow"

---

## 작업의 목적

Agent V2 세션에서 실시간 메시지 표시가 불완전한 문제를 해결하여, `idea.md`에 정의된 "React owns the message stack" 아키텍처를 완전히 구현합니다.

**핵심 원칙 (from idea.md):**

```
useAgentSession --> useAgentSession: updateMessage(messages)  // React가 메시지 스택 직접 업데이트
useAgentSession -> AgentSessionManager: pushMessage(newMessage)  // 완료 후 Rust에 알림
```

---

## 현재의 상태 / 문제점

### 증상

1. **User 메시지**: Send 버튼 클릭 시 UI에 즉시 표시되지 않음
2. **Assistant 메시지**: Streaming 중에는 보이지만 완료 후 사라짐
3. **Tool result 메시지**: Tool 실행 후 UI에 표시되지 않음

### 원인 분석

#### 1. LLMServiceContext - finalMessage 미반영

**현재 코드** (`src/context/LLMServiceContext.tsx:288-320`):

```typescript
// Create final message
const finalMessage: Message = {
  id: streamingMessage.id ?? `msg_${Date.now()}`,
  sessionId,
  role: 'assistant',
  content: fullContent ? [{ type: 'text' as const, text: fullContent }] : [],
  tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
  thinking: thinkingContent || undefined,
  // ❌ isStreaming 플래그 없음
};

// ❌ streamingMessages에 set하지 않음!
// setTimeout으로 100ms 후 delete만 함
setTimeout(() => {
  setStreamingMessages((prev) => {
    const next = new Map(prev);
    next.delete(sessionId);
    return next;
  });
}, 100);
```

**문제:**

- `finalMessage`에 `isStreaming: false` 플래그가 없음
- `streamingMessages`에 set하지 않아 `AgentChatContext` effect가 감지 불가
- 100ms setTimeout은 race condition 유발 (effect 실행 전 clear 가능)

#### 2. AgentChatContext - User 메시지 미추가

**현재 코드** (`src/context/AgentChatContext.tsx:250-280`):

```typescript
const submit = useCallback(async (message: Message) => {
  setIsLoading(true);
  setError(null);

  // ❌ setMessages에 추가하지 않음
  await invoke('agent_send_message', {
    request: { sessionId: currentSession.id, message },
  });

  // Rust가 DB에 저장만 하고 React state는 업데이트 안됨
});
```

**문제:**

- User 메시지가 Rust DB에만 저장되고 React state에 추가되지 않음
- `idea.md`의 optimistic update 패턴 미구현

#### 3. AgentChatContext - Tool result 동기화 없음

**현재 코드** (`src/context/AgentChatContext.tsx:180-220`):

```typescript
// agent:event 리스너에서 MessageAdded 이벤트 처리 없음
if (eventType === 'StatusChanged') { ... }
else if (eventType === 'WorkflowError') { ... }
else if (eventType === 'WorkflowCompleted') { ... }
// ❌ MessageAdded 처리 없음
```

**문제:**

- Rust가 tool result를 DB에 저장하지만 React에 알리지 않음
- `elaborated_idea.md` Section 4.2에 정의된 `agent:event(MessageAdded)` 미구현

### 아키텍처 관점

**현재 구현:**

```
LLMService: streaming → finalMessage 생성 → (streamingMessages에 set 안함) → setTimeout clear
AgentChatContext: effect가 감지 못함 → messages 배열에 추가 안됨
```

**idea.md 설계 (목표):**

```
LLMService: streaming → finalMessage (isStreaming: false) → streamingMessages.set()
AgentChatContext: effect 감지 → setMessages([...prev, finalMessage])
```

---

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. ✅ User 메시지가 Send 버튼 클릭 즉시 UI에 표시됨
2. ✅ Streaming 중 assistant 메시지가 실시간으로 표시됨
3. ✅ Streaming 완료 후 assistant 메시지가 사라지지 않고 유지됨
4. ✅ Tool result 메시지가 실행 후 UI에 표시됨
5. ✅ 세션 전환 후 복귀 시 모든 메시지 정상 표시 (기존 기능 유지)

### 테스트 시나리오

1. **Simple Chat**: User 메시지 → Assistant 응답 (no tool call)
   - User 메시지 즉시 표시 확인
   - Streaming 중 표시 확인
   - 완료 후 메시지 유지 확인

2. **Tool Call Workflow**: User 메시지 → Assistant (with tool call) → Tool result → Assistant
   - 모든 메시지가 순서대로 표시되는지 확인
   - Tool result가 UI에 보이는지 확인

3. **Session Resume**: 워크플로우 중 다른 세션으로 전환 후 복귀
   - 기존 메시지 모두 로드되는지 확인
   - 백그라운드 진행된 메시지도 표시되는지 확인

---

## 수정이 필요한 코드 및 스니핏

### 1. LLMServiceContext.tsx - finalMessage 반영

**위치**: `src/context/LLMServiceContext.tsx:288-320`

**수정 내용**:

```typescript
// Create final message with isStreaming: false
const finalMessage: Message = {
  id: streamingMessage.id ?? `msg_${Date.now()}`,
  sessionId,
  threadId: sessionId,
  role: 'assistant',
  content: fullContent ? [{ type: 'text' as const, text: fullContent }] : [],
  createdAt: new Date(),
  tool_calls: toolCalls.length > 0 ? toolCalls : undefined,
  thinking: thinkingContent || undefined,
  isStreaming: false, // ✅ 명시적 완료 표시
};

logger.info('Completion request completed', {
  sessionId,
  contentLength: fullContent.length,
  toolCallCount: toolCalls.length,
});

// ✅ Set finalMessage to trigger AgentChatContext effect
setStreamingMessages((prev) => {
  const next = new Map(prev);
  next.set(sessionId, finalMessage);
  return next;
});

// ✅ Remove setTimeout - effect will handle persistence
// AgentChatContext will add to messages array
// Cleanup happens naturally on next request or session unmount

// Update status to idle
updateSessionStatus(sessionId, 'idle');
```

### 2. AgentChatContext.tsx - User 메시지 Optimistic Update

**위치**: `src/context/AgentChatContext.tsx:250-280`

**수정 내용**:

```typescript
const submit = useCallback(
  async (message: Message) => {
    if (!currentSession?.id) {
      logger.error('Cannot submit: no active session');
      return;
    }

    logger.info('Submitting message to agent workflow', {
      sessionId: currentSession.id,
      messageId: message.id,
    });

    try {
      setIsLoading(true);
      setError(null);

      // ✅ Optimistic update - add user message immediately (idea.md pattern)
      setMessages((prev) => [...prev, message]);

      // Delegate to Rust backend (Rust will save the message to DB)
      await invoke('agent_send_message', {
        request: {
          sessionId: currentSession.id,
          message,
        },
      });

      logger.info('Message submitted successfully', {
        sessionId: currentSession.id,
        messageId: message.id,
      });
    } catch (err) {
      logger.error('Failed to submit message', err);
      setError(err instanceof Error ? err.message : String(err));
      setIsLoading(false);

      // ✅ Rollback on error
      setMessages((prev) => prev.filter((m) => m.id !== message.id));
    }
  },
  [currentSession?.id],
);
```

### 3. AgentChatContext.tsx - MessageAdded 이벤트 처리

**위치**: `src/context/AgentChatContext.tsx:195-220` (agent:event 리스너 내부)

**수정 내용**:

```typescript
// Handle different event types
if (eventType === 'StatusChanged') {
  const status = payload.status as SessionStatus;
  if (status === 'Idle') {
    setWorkflowStatus('idle');
    setIsLoading(false);
  } else if (status === 'Busy') {
    setWorkflowStatus('busy');
    setIsLoading(true);
  } else if (status === 'Paused') {
    setWorkflowStatus('paused');
    setIsLoading(false);
  }
} else if (eventType === 'WorkflowError') {
  setWorkflowStatus('error');
  setIsLoading(false);
  setError((payload.error as string) ?? 'Unknown error');
} else if (eventType === 'MessageAdded') {
  // ✅ Handle tool result messages from Rust
  const newMessage = payload.message as Message;

  logger.debug('MessageAdded event received', {
    sessionId: currentSession.id,
    messageId: newMessage.id,
    role: newMessage.role,
  });

  // Add to messages if not already present
  setMessages((prev) => {
    const exists = prev.some((m) => m.id === newMessage.id);
    if (exists) return prev;
    return [...prev, newMessage];
  });
} else if (eventType === 'WorkflowCompleted') {
  // Workflow completed - messages already in React state per idea.md
  setWorkflowStatus('idle');
  setIsLoading(false);

  logger.info('Workflow completed', {
    sessionId: currentSession.id,
    messageCount: messages.length,
  });

  // No DB reload needed - React owns message state (idea.md architecture)
  // Tool results and assistant responses already added via streaming
}
```

---

## 재사용 가능한 연관 코드

### Message 타입 정의

**파일**: `src/models/chat.ts`
**인터페이스**:

```typescript
interface Message {
  id: string;
  sessionId: string;
  threadId?: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: MessageContent[];
  tool_calls?: ToolCall[];
  thinking?: string;
  isStreaming?: boolean; // ← 이 플래그 활용
  createdAt?: Date;
}
```

### 참고: use-ai-service의 isStreaming 사용 예

**파일**: `src/hooks/use-ai-service.ts:340-360`

```typescript
// Streaming 중
finalMessage = {
  id: currentResponseId,
  isStreaming: true, // ← Streaming 중
  content: stringToMCPContentArray(fullContent),
  // ...
};

// 완료 시
finalMessage = {
  id: currentResponseId,
  isStreaming: false, // ← 완료
  content: stringToMCPContentArray(fullContent),
  // ...
};
```

### 기존 Session Resume 로직 (유지)

**파일**: `src/context/AgentChatContext.tsx:145-175`

```typescript
useEffect(() => {
  if (!currentSession?.id) {
    setMessages([]);
    return;
  }

  const loadMessages = async () => {
    const page = await getMessagesPageForSession(
      currentSession.id,
      currentSession.id,
      1,
      1000,
    );
    setMessages(page.items); // ← 이 로직은 그대로 유지
  };

  loadMessages();
}, [currentSession?.id]);
```

---

## Test Code 추가 및 수정 가이드

### Unit Test 추가

**파일**: `src/context/__tests__/AgentChatContext.test.tsx` (신규)

**테스트 케이스**:

```typescript
describe('AgentChatContext - Message State Management', () => {
  it('should add user message optimistically on submit', async () => {
    // submit() 호출 시 messages 배열에 즉시 추가되는지 확인
  });

  it('should rollback user message on submit error', async () => {
    // invoke() 실패 시 messages에서 제거되는지 확인
  });

  it('should add assistant message when isStreaming becomes false', async () => {
    // streamingMessages에 isStreaming: false가 set되면
    // useEffect가 messages에 추가하는지 확인
  });

  it('should add tool result via MessageAdded event', async () => {
    // agent:event(MessageAdded) 수신 시 messages에 추가되는지 확인
  });

  it('should not duplicate messages', async () => {
    // 같은 id의 메시지가 중복 추가되지 않는지 확인
  });
});
```

### Integration Test

**파일**: `src/context/__tests__/LLMServiceContext.integration.test.tsx`

**시나리오**:

1. LLMServiceContext가 streaming 완료 시 `isStreaming: false` set
2. AgentChatContext effect가 이를 감지하여 messages 추가
3. Rust로 `agent_handle_llm_response` 전송

### E2E Test (Manual)

**시나리오**: Tool call이 있는 워크플로우

1. User 입력: "현재 날짜를 확인하고 메모장에 저장해줘"
2. 확인:
   - User 메시지 즉시 표시
   - Assistant streaming 표시
   - Tool call (get_date) result 표시
   - Tool call (save_to_notepad) result 표시
   - 최종 Assistant 응답 표시

---

## 추가 분석 과제

### 1. MessageAdded 이벤트 타이밍

**질문**: Rust가 MessageAdded 이벤트를 언제 emit하는가?

- Tool result 저장 직후?
- Assistant 메시지 저장 직후? (이 경우 중복 추가 방지 필요)

**분석 대상**: `src-tauri/src/agents/session_manager.rs`

- `handle_llm_response` 함수
- `emit_agent_event` 호출 위치

### 2. streamingMessages 정리 시점

**현재 문제**: setTimeout(100ms)는 부적절
**검토 필요**:

- AgentChatContext effect에서 messages 추가 후 명시적으로 clear?
- 다음 요청 시 자연스럽게 덮어쓰기?
- Session unmount 시 cleanup?

**권장**: 다음 요청 시 덮어쓰기 (가장 간단하고 안전)

### 3. Resume 시 Workflow 상태 복원

**질문**: 세션 전환 후 복귀 시 background에서 진행된 메시지는?
**현재 구현**: `currentSession.id` 변경 → DB에서 reload (✅ 정상 작동)
**추가 검증**: Background workflow 중 세션 전환 후 복귀 테스트

---

## Clarification Q-list

### Q1: MessageAdded 이벤트의 범위

Rust가 MessageAdded 이벤트를 모든 메시지 타입(user, assistant, tool)에 대해 emit하는가, 아니면 tool result만?

**영향**:

- 모든 타입이면: 중복 추가 방지 로직 필수
- Tool result만이면: 현재 구현으로 충분

**확인 방법**: `src-tauri/src/agents/session_manager.rs` 분석

### Q2: streamingMessages 정리 전략

어느 시점에 streamingMessages를 clear하는 것이 안전한가?

**옵션**:

- A. Effect에서 messages 추가 후 즉시 clear
- B. 다음 streaming 시작 시 덮어쓰기
- C. WorkflowCompleted 이벤트에서 clear

**권장**: B (가장 간단하며 race condition 없음)

### Q3: Optimistic Update Rollback 범위

User 메시지 submit 실패 시 rollback 외에 추가로 처리할 사항은?

**고려 사항**:

- Error 메시지 표시?
- Retry 메커니즘?
- Rust DB에 저장되었지만 React state에 없는 경우?

---

## 참고 문서 매핑

| 문서                 | 섹션                                 | 관련 내용                                                                                          |
| -------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `idea.md`            | "요청 전달 및 Agentic Flow 시작"     | `useAgentSession --> useAgentSession: updateMessage(messages)` - React가 메시지 스택 직접 업데이트 |
| `elaborated_idea.md` | Section 2.B "Frontend Service Layer" | "React owns the message stack" 원칙 정의                                                           |
| `elaborated_idea.md` | Section 3.3 "LLM Integration"        | Streaming 완료 시 `isStreaming: false` 설정 단계                                                   |
| `elaborated_idea.md` | Section 4.2 "Starting a Workflow"    | User 메시지 optimistic update, Tool result MessageAdded 처리                                       |

---

**작성자**: GitHub Copilot Agent
**검토 필요 사항**: Clarification Q-list 참조
