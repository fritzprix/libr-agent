# Gemini Service 메시지 검증 단순화 리팩토링 계획

## 작업의 목적

Gemini AI 서비스에서 발생하는 메시지 순서 검증 오류를 해결하고, 과도하게 복잡한 검증 로직을 단순화하여 안정성과 유지보수성을 향상시킨다.

## 현재의 상태 / 문제점

### 1. 메시지 순서 규칙 위반
- **로그 분석**: `messageRoles: ["assistant","tool","assistant","tool",...]` 패턴이 나타남
- **근본 원인**: Gemini는 반드시 "user" 메시지로 시작해야 하는데, "assistant"로 시작하는 메시지 배열을 전송하고 있음
- **현재 에러**: "All validation strategies failed - refusing to proceed"

### 2. tool 역할 변환 누락
- **기존 설계**: tool 메시지를 user 역할로 변환하여 Gemini 호환성 확보 예정이었음
- **현재 상황**: 변환이 적용되지 않아 `tool` 역할이 그대로 유지됨
- **결과**: Gemini에서 지원하지 않는 메시지 순서 패턴 발생

### 3. 과도하게 복잡한 검증 로직
- **현재 구조**: 4단계 검증 전략 (Strategy 0~4)
  - Strategy 1: `findValidSequenceFromEnd` - 끝에서부터 유효한 시퀀스 찾기
  - Strategy 2: `repairMessageSequence` - 문제 있는 tool call 제거
  - Strategy 3: `extractTextOnlySequence` - 텍스트 메시지만 추출
  - Strategy 4: 최소 사용자 메시지 유지
- **문제점**: 근본 원인(tool → user 변환 누락)을 해결하지 않고 복잡한 우회 로직만 추가

### 4. 불안정한 ID 생성
```typescript
private generateDeterministicToolCallId(functionCall: FunctionCall): string {
  const argsStr = JSON.stringify(functionCall.args || {}); // 키 순서 불안정
  // ...
}
```
- **문제**: `JSON.stringify`는 객체 키 순서를 보장하지 않아 동일한 인자에 대해 다른 ID가 생성될 수 있음

## 변경 이후의 상태 / 해결 판정 기준

### 1. 메시지 순서 정상화
- **목표**: `["user","assistant","user","assistant",...]` 패턴 달성
- **판정 기준**: 로그에서 "assistant"로 시작하는 메시지 배열 제거

### 2. 단순화된 검증 로직
- **목표**: 복잡한 4단계 전략을 단순한 2단계로 축소
  1. tool → user 역할 변환
  2. 첫 번째 user 메시지부터 시작하도록 슬라이싱
- **판정 기준**: 검증 관련 메서드 4개 제거 가능

### 3. 안정적인 ID 생성
- **목표**: 동일한 함수 호출에 대해 항상 동일한 ID 생성
- **판정 기준**: 키 정렬된 JSON 직렬화로 안정성 확보

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 1. validateGeminiMessageStack 메서드 단순화

**현재 코드 (라인 178-223):**
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

**수정 후 코드:**
```typescript
private validateGeminiMessageStack(messages: Message[]): Message[] {
  if (messages.length === 0) {
    return messages;
  }

  const logger = getLogger('GeminiValidation');
  
  // 1단계: tool → user 역할 변환 (Gemini 호환성을 위해)
  const convertedMessages = messages.map(m => {
    if (m.role === 'tool') {
      return { ...m, role: 'user' as const };
    }
    return m;
  });

  // 2단계: 첫 번째 user 메시지부터 시작하도록 조정
  const firstUserIndex = convertedMessages.findIndex(msg => msg.role === 'user');
  if (firstUserIndex === -1) {
    logger.warn('No user message found after role conversion');
    return [];
  }

  const validMessages = convertedMessages.slice(firstUserIndex);
  
  logger.info(`Role conversion and validation: ${messages.length} → ${validMessages.length} messages`, {
    originalRoles: messages.map(m => m.role),
    convertedRoles: validMessages.map(m => m.role)
  });

  return validMessages;
}
```

### 2. generateDeterministicToolCallId 메서드 안정화

**현재 코드 (라인 24-38):**
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

**수정 후 코드:**
```typescript
private generateDeterministicToolCallId(functionCall: FunctionCall): string {
  // 키 정렬된 안정적인 JSON 직렬화
  const stableStringify = (obj: unknown): string => {
    if (obj === null || typeof obj !== 'object') return JSON.stringify(obj);
    if (Array.isArray(obj)) return '[' + obj.map(stableStringify).join(',') + ']';
    const keys = Object.keys(obj as Record<string, unknown>).sort();
    return '{' + keys.map(k => `${JSON.stringify(k)}:${stableStringify((obj as any)[k])}`).join(',') + '}';
  };

  const argsStr = stableStringify(functionCall.args || {});
  const content = `${functionCall.name}:${argsStr}`;

  let hash = 0;
  for (let i = 0; i < content.length; i++) {
    const char = content.charCodeAt(i);
    hash = (hash << 5) - hash + char;
    hash |= 0;
  }

  return `tool_${Math.abs(hash).toString(36)}`;
}
```

### 3. 제거할 메서드들

**제거 대상 메서드들 (라인 227-357):**
```typescript
// 이 메서드들은 더 이상 필요하지 않음 - 완전 제거
private findValidSequenceFromEnd(messages: Message[]): Message[] { ... }
private repairMessageSequence(messages: Message[]): Message[] { ... }  
private extractTextOnlySequence(messages: Message[]): Message[] { ... }
private isValidGeminiSequence(messages: Message[]): boolean { ... }
```

### 4. convertToGeminiMessages에서 tool 역할 처리 개선

**현재 코드 (라인 385-410):**
```typescript
} else if (m.role === 'tool') {
  // Find the corresponding assistant message to get the function name
  let functionName: string | undefined;
  for (let j = messages.indexOf(m) - 1; j >= 0; j--) {
    const prevMessage = messages[j];
    if (prevMessage.role === 'assistant' && prevMessage.tool_calls) {
      const toolCall = prevMessage.tool_calls.find(
        (tc) => tc.id === m.tool_call_id,
      );
      if (toolCall) {
        functionName = toolCall.function.name;
        break;
      }
    }
  }
```

**수정 후 코드:**
```typescript
} else if (m.role === 'tool') {
  // tool 메시지는 이미 validateGeminiMessageStack에서 user로 변환되었으므로
  // 이 분기는 실행되지 않아야 함
  logger.warn('Unexpected tool message in convertToGeminiMessages - should have been converted to user');
  continue;
```

## 기대 효과

1. **안정성 향상**: 메시지 순서 오류 근본 해결
2. **코드 단순화**: 복잡한 검증 로직 제거로 유지보수성 향상
3. **성능 개선**: 불필요한 검증 단계 제거
4. **디버깅 용이성**: 명확한 로그와 단순한 로직으로 문제 추적 개선
