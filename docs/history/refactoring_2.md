# Refactoring Pla### 2. 불안정한 Tool Call ID 생성

**Gemini 서비스에서의 문제:**

```typescript
const sessionSalt = Date.now().toString(36).slice(-4);
return `tool_${Math.abs(hash).toString(36)}_${sessionSalt}`;
```

**ChatContext에서의 Tool Call ID 매핑:**

```typescript
const toolResultMessage: Message = {
  id: createId(),
  assistantId: currentAssistant?.id,
  role: 'tool',
  content: serialized.text || '...',
  uiResource: serialized.uiResource,
  tool_call_id: toolCall.id,  // 여기서 원본 toolCall.id 사용
  sessionId: currentSession?.id || '',
};
```

- **핵심 문제**: Gemini 서비스의 `Date.now()` 기반 salt로 인해 같은 함수+인자라도 다른 ID가 생성되어, `ToolCaller`에서 `tool_call_id` 매핑이 실패할 수 있음
- **매핑 과정**: Assistant 메시지의 `tool_calls[].id` → Tool result 메시지의 `tool_call_id`로 직접 복사되므로 ID 안정성이 중요
- **orphaned messages 증가**: ID 불일치로 인해 tool result가 orphaned되어 Gemini 검증에서 제거됨
- **연쇄 실패**: Tool result orphaning → 메시지 검증 실패 → 전체 대화 제거 → fallback 메시지 주입 순으로 문제 확산 메시지 검증 로직 개선

## 작업의 목적

Gemini AI 서비스에서 메시지 스택 검증 시 과도하게 메시지를 제거하여 빈 대화가 되고, 이로 인해 fallback 메시지가 생성되어 무한 도구 호출이 발생하는 문제를 해결한다. 메시지 시퀀스를 보다 정교하게 검증하고 복구하여 대화 맥락을 최대한 보존하면서도 Gemini의 sequencing 규칙을 준수하도록 개선한다.

## 현재의 상태 / 문제점

### 1. 과도한 메시지 제거 (이전 문제)

```log
[2025-08-17][02:36:35][webview][INFO] [AIService] Removed 10 assistant messages that violated Gemini sequencing rules {"originalCount":20,"validatedCount":0,"removedToolCallIds":["tool_gdms18_r2mf",...]}
```

- `validateGeminiMessageStack`이 순차적으로 메시지를 검증하며 하나라도 위반되면 제거
- 연쇄적으로 많은 메시지가 제거되어 전체 대화가 빈 상태가 됨

### 1-2. 새로운 문제 패턴 (수정 후 발생)

```log
[2025-08-17][03:56:07][webview][ERROR] [GeminiValidation] All validation strategies failed - refusing to proceed
[2025-08-17][03:56:07][webview][ERROR] [useAIService] Error in useAIService stream: {"provider":"gemini","name":"AIServiceError"}
[2025-08-17][03:56:07][webview][ERROR] [ChatContext] Message submission failed
```

- **수정 후 나타난 새로운 문제**: 검증 전략이 너무 엄격해져서 정상적인 tool calling 시퀀스도 차단
- **Tool 실행은 성공**: `tool execution completed {"success":true}`이지만 AI 응답 생성에서 실패
- **무한 루프는 방지됨**: 하지만 legitimate한 대화도 중단됨

### 2. 불안정한 Tool Call ID 생성

```typescript
const sessionSalt = Date.now().toString(36).slice(-4);
return `tool_${Math.abs(hash).toString(36)}_${sessionSalt}`;
```

- `Date.now()` 기반 salt로 인해 같은 함수+인자라도 다른 ID 생성
- Tool call과 tool result 간 매칭 실패로 orphaned messages 증가

### 3. 위험한 Fallback 동작

```typescript
return [
  {
    id: 'fallback-user-msg',
    role: 'user',
    content: 'Please continue.',
    // ...
  },
];
```

- 모든 메시지 제거 시 임의 user 메시지 주입
- Aggressive한 system prompt와 결합되어 무한 도구 호출 발생

### 4. Tool Call ID 중복 가능성

**분석된 추가 문제:**

```typescript
// normalizeToolResult 함수에서
const id = `tool-${toolName}-${Date.now()}`;

// ChatContext의 ToolCaller에서
const toolResultMessage: Message = {
  id: createId(),  // 새로운 Message ID
  tool_call_id: toolCall.id,  // 원본 Tool Call ID
};
```

- **ID 혼재**: MCPResponse의 `id`와 Message의 `tool_call_id`가 서로 다른 생성 방식 사용
- **타이밍 의존성**: `Date.now()` 기반 ID들이 동시 실행 시 중복될 가능성
- **검증 실패 원인**: Tool call ID 불일치로 orphaned tool result 메시지 대량 생성

## 변경 이후의 상태 / 해결 판정 기준

### 성공 기준

1. 메시지 검증 시 대화 맥락 최대한 보존 (최소 3-5개 메시지 유지)
2. Tool call ID의 결정적 생성으로 매칭 안정성 확보
3. Fallback 시 에러 발생으로 무한 루프 방지
4. Gemini sequencing 규칙 완전 준수
5. **정상적인 tool calling 시퀀스는 중단되지 않음** (새로 추가)

### 검증 방법

1. 로그에서 "All messages were removed" 경고 사라짐 ✅ (달성됨)
2. Tool call과 result 간 orphaned message 최소화
3. 무한 `startCube` 호출 없이 정상 대화 진행 ✅ (달성됨)
4. 복잡한 도구 호출 시나리오에서 안정적 동작
5. **"All validation strategies failed" 에러가 legitimate한 대화에서 발생하지 않음** (새로 추가)

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. `src/lib/ai-service/gemini.ts` - 검증 로직 완화 (최소 수정)

**핵심 문제**: 현재 `isValidGeminiSequence`가 정상적인 연속 tool calling을 잘못 차단

**현재 코드:**

```typescript
/**
 * Check if a message sequence follows Gemini's rules
 */
private isValidGeminiSequence(messages: Message[]): boolean {
  if (messages.length === 0) return false;

  // Must start with user message
  if (messages[0].role !== 'user') return false;

  // Check alternating pattern and function call rules
  for (let i = 0; i < messages.length; i++) {
    const current = messages[i];
    const previous = i > 0 ? messages[i - 1] : null;

    // Assistant function calls must follow user or tool messages
    if (
      current.role === 'assistant' &&
      current.tool_calls &&
      current.tool_calls.length > 0
    ) {
      if (!previous || (previous.role !== 'user' && previous.role !== 'tool')) {
        return false; // 여기서 정상적인 연속 tool call도 차단!
      }
    }

    // Tool messages must follow assistant messages with tool calls
    if (current.role === 'tool') {
      if (!previous || previous.role !== 'assistant' || !previous.tool_calls) {
        return false;
      }
    }
  }

  return true;
}
```

**수정 후 코드:**

```typescript
/**
 * Check if a message sequence follows Gemini's rules (완화됨)
 */
private isValidGeminiSequence(messages: Message[]): boolean {
  if (messages.length === 0) return false;

  // Must start with user message
  if (messages[0].role !== 'user') return false;

  // Check Gemini sequencing rules (완화된 버전)
  for (let i = 1; i < messages.length; i++) {
    const current = messages[i];
    const previous = messages[i - 1];

    // Assistant messages can follow user, tool, or assistant messages
    if (current.role === 'assistant') {
      // Assistant는 user, tool, assistant 다음에 올 수 있음 (연속 가능)
      if (previous.role !== 'user' && previous.role !== 'tool' && previous.role !== 'assistant') {
        return false;
      }
    }

    // Tool messages must follow assistant messages with tool calls
    if (current.role === 'tool') {
      if (previous.role !== 'assistant' || !previous.tool_calls || previous.tool_calls.length === 0) {
        return false;
      }
    }

    // User messages can follow any role (always valid)
  }

  return true;
}
```

### 2. `src/lib/ai-service/gemini.ts` - 검증 전략 순서 조정 (최소 수정)

**현재 코드:**

```typescript
private validateGeminiMessageStack(messages: Message[]): Message[] {
  if (messages.length === 0) {
    return messages;
  }

  const logger = getLogger('GeminiValidation');
  
  // Strategy 1: Try to find the longest valid sequence from the end
  const validSequenceFromEnd = this.findValidSequenceFromEnd(messages);
  if (validSequenceFromEnd.length > 0) {
    logger.info(`Found valid sequence of ${validSequenceFromEnd.length} messages from end`);
    return validSequenceFromEnd;
  }

  // Strategy 2: Try to repair the sequence by removing only problematic tool calls
  const repairedSequence = this.repairMessageSequence(messages);
  if (repairedSequence.length > 0) {
    logger.info(`Repaired sequence with ${repairedSequence.length} messages`);
    return repairedSequence;
  }

  // Strategy 3: Keep only user and assistant text messages (no tool calls)
  const textOnlySequence = this.extractTextOnlySequence(messages);
  if (textOnlySequence.length > 0) {
    logger.warn(`Fallback to text-only sequence with ${textOnlySequence.length} messages`);
    return textOnlySequence;
  }

  // Last resort: throw error instead of creating artificial user message
  logger.error('All validation strategies failed - refusing to proceed');
  throw new AIServiceError(
    'Message sequence validation failed: unable to create valid Gemini conversation from provided messages',
    AIServiceProvider.Gemini,
  );
}
```

**수정 후 코드:**

```typescript
private validateGeminiMessageStack(messages: Message[]): Message[] {
  if (messages.length === 0) {
    return messages;
  }

  const logger = getLogger('GeminiValidation');
  
  // Strategy 0: 원본이 이미 유효한지 먼저 확인 (새로 추가)
  if (this.isValidGeminiSequence(messages)) {
    logger.debug('Original message sequence is already valid');
    return messages;
  }

  // Strategy 1: Try to find the longest valid sequence from the end
  const validSequenceFromEnd = this.findValidSequenceFromEnd(messages);
  if (validSequenceFromEnd.length > 0) {
    logger.info(`Found valid sequence of ${validSequenceFromEnd.length} messages from end`);
    return validSequenceFromEnd;
  }

  // Strategy 2: Try to repair the sequence by removing only problematic tool calls
  const repairedSequence = this.repairMessageSequence(messages);
  if (repairedSequence.length > 0) {
    logger.info(`Repaired sequence with ${repairedSequence.length} messages`);
    return repairedSequence;
  }

  // Strategy 3: Keep only user and assistant text messages (no tool calls)
  const textOnlySequence = this.extractTextOnlySequence(messages);
  if (textOnlySequence.length > 0) {
    logger.warn(`Fallback to text-only sequence with ${textOnlySequence.length} messages`);
    return textOnlySequence;
  }

  // Strategy 4: 최소 사용자 메시지라도 유지 (새로 추가)
  const lastUserMessage = messages.slice().reverse().find(msg => msg.role === 'user');
  if (lastUserMessage) {
    logger.warn('Creating minimal conversation from last user message');
    return [lastUserMessage];
  }

  // Last resort: throw error with detailed reason
  logger.error('All validation strategies failed - refusing to proceed', {
    messageCount: messages.length,
    messageRoles: messages.map(m => m.role)
  });
  throw new AIServiceError(
    'Message sequence validation failed: unable to create valid Gemini conversation from provided messages',
    AIServiceProvider.Gemini,
  );
}
```

### 3. `src/lib/ai-service/gemini.ts` - Tool Call ID 생성 안정화 (기존 함수 수정)

**현재 코드:**

```typescript
private generateDeterministicToolCallId(functionCall: FunctionCall): string {
  const argsStr = JSON.stringify(functionCall.args || {});
  const content = `${functionCall.name}:${argsStr}`;

  let hash = 0;
  for (let i = 0; i < content.length; i++) {
    const char = content.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32-bit integer
  }

  // Convert to base36 and add timestamp for some uniqueness within the session
  const sessionSalt = Date.now().toString(36).slice(-4);
  return `tool_${Math.abs(hash).toString(36)}_${sessionSalt}`;
}
```

**수정 후 코드:**

```typescript
private generateDeterministicToolCallId(functionCall: FunctionCall): string {
  // Create a deterministic ID based on function name and arguments only
  const argsStr = JSON.stringify(functionCall.args || {});
  const content = `${functionCall.name}:${argsStr}`;

  // Use a simple hash to create a shorter, deterministic ID
  let hash = 0;
  for (let i = 0; i < content.length; i++) {
    const char = content.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash = hash & hash; // Convert to 32-bit integer
  }

  // Remove timestamp-based salt for true determinism
  return `tool_${Math.abs(hash).toString(36)}`;
}
```

## 핵심 변경사항 요약

### ✅ 최소 수정 원칙
1. **기존 함수 수정**: 새로운 복잡한 메서드 추가 대신 기존 로직 개선
2. **판정 기준 완화**: Assistant 연속 메시지를 유효하다고 인정
3. **원본 우선**: 이미 유효한 시퀀스는 건드리지 않음

### ✅ 핵심 문제 해결
- **정상적인 tool calling 연속** → 이제 차단되지 않음
- **무한 루프 방지** → 여전히 유지됨
- **복잡도 최소화** → 기존 구조 그대로 활용
```
