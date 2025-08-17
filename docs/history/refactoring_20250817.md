# Refactoring Plan: GeminiService Simplification and Overengineering Removal

## 작업의 목적

GeminiService 클래스에서 불필요한 복잡성을 제거하고 코드 품질을 향상시킵니다. 현재 결정론적 ID 생성 로직이 과도하게 복잡하며, 프로젝트 요구사항상 중복 호출 방지나 idempotency가 필요하지 않으므로 단순한 고유 ID로 교체하여 코드를 단순화합니다.

## 현재의 상태 / 문제점

### 1. Overengineered ID 생성 로직

- `generateDeterministicToolCallId` 메서드에서 수동으로 구현한 stable stringify + 해시 함수
- 복잡한 객체 직렬화 로직이 메서드 내부에 인라인으로 구현됨
- 재사용성이 없고 테스트하기 어려운 구조

### 2. 다중 로거 인스턴스

- 파일 내에서 여러 `getLogger()` 호출로 컨텍스트가 분산됨
- 일관성 없는 로거 컨텍스트 이름 사용

### 3. 메서드 내부 타입 선언

- `GeminiServiceConfig` 인터페이스가 `streamChat` 메서드 내부에 선언됨
- 타입 재사용성과 가독성 저하

### 4. JSON.parse 안전성 부족

- `convertToGeminiMessages`에서 `JSON.parse(tc.function.arguments)` 호출 시 예외 처리 없음
- 파싱 실패 시 전체 스트림이 중단될 수 있음

## 변경 이후의 상태 / 해결 판정 기준

### 1. 단순화된 ID 생성

- `paralleldriver.createId()`를 사용한 간단한 고유 ID 생성
- 복잡한 결정론적 로직 제거
- 메서드 길이 90% 단축

### 2. 통일된 로거 사용

- 파일 최상단에 단일 모듈 로거 선언
- 일관된 컨텍스트 이름 'GeminiService' 사용

### 3. 타입 분리

- `GeminiServiceConfig` 인터페이스를 파일 상단으로 이동
- 타입 재사용성 향상

### 4. 안전한 JSON 파싱

- `tryParse` 유틸리티 함수 추가
- 파싱 실패 시 graceful degradation

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 변경 전 (문제가 있는 코드)

```typescript
// 1. 복잡한 결정론적 ID 생성
private generateDeterministicToolCallId(functionCall: FunctionCall): string {
  // 키 정렬된 안정적인 JSON 직렬화
  const stableStringify = (obj: unknown): string => {
    if (obj === null || typeof obj !== 'object') return JSON.stringify(obj);
    if (Array.isArray(obj)) return '[' + obj.map(stableStringify).join(',') + ']';
    const keys = Object.keys(obj as Record<string, unknown>).sort();
    return '{' + keys.map(k => `${JSON.stringify(k)}:${stableStringify((obj as Record<string, unknown>)[k])}`).join(',') + '}';
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

// 2. 다중 로거 인스턴스
const logger = getLogger('AIService'); // 파일 상단
const logger = getLogger('GeminiValidation'); // validateGeminiMessageStack 내부
const logger = getLogger('GeminiToolResponse'); // convertToGeminiMessages 내부
const logger = getLogger('GeminiToolResponseStats'); // logToolResponseStats 내부

// 3. 메서드 내부 타입 선언
async *streamChat(...) {
  // ...
  interface GeminiServiceConfig {
    responseMimeType: string;
    tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
    systemInstruction?: Array<{ text: string }>;
    maxOutputTokens?: number;
    temperature?: number;
  }
  // ...
}

// 4. 안전하지 않은 JSON.parse
parts: m.tool_calls.map((tc) => ({
  functionCall: {
    name: tc.function.name,
    args: JSON.parse(tc.function.arguments), // 예외 처리 없음
  },
})),
```

### 변경 후 (개선된 코드)

```typescript
import { createId } from '@parallel-drive/gpt3-tokenizer'; // paralleldriver 패키지에서 가져오기
// ... 기존 imports

// 파일 상단에 통일된 로거와 타입 선언
const logger = getLogger('GeminiService');

interface GeminiServiceConfig {
  responseMimeType: string;
  tools?: Array<{ functionDeclarations: FunctionDeclaration[] }>;
  systemInstruction?: Array<{ text: string }>;
  maxOutputTokens?: number;
  temperature?: number;
}

// 안전한 JSON 파싱 유틸리티
function tryParse<T = unknown>(input?: string): T | undefined {
  if (!input) return undefined;
  try {
    return JSON.parse(input) as T;
  } catch {
    return undefined;
  }
}

export class GeminiService extends BaseAIService {
  // ...

  // 1. 단순화된 ID 생성
  private generateToolCallId(): string {
    return `tool_${createId()}`;
  }

  // 2. 스트림에서 사용
  yield JSON.stringify({
    tool_calls: chunk.functionCalls.map((fc: FunctionCall) => {
      const toolCallId = this.generateToolCallId(); // 단순화된 호출

      logger.debug('Generated tool call ID', {
        functionName: fc.name,
        toolCallId,
      });

      return {
        id: toolCallId,
        type: 'function',
        function: {
          name: fc.name,
          arguments: JSON.stringify(fc.args),
        },
      };
    }),
  });

  // 3. 안전한 JSON 파싱 적용
  parts: m.tool_calls.map((tc) => {
    const args = tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
    return {
      functionCall: {
        name: tc.function.name,
        args,
      },
    };
  }),

  // 4. 통일된 로거 사용 (모든 메서드에서 파일 상단의 logger 재사용)
  private validateGeminiMessageStack(messages: Message[]): Message[] {
    if (messages.length === 0) {
      return messages;
    }
    
    // getLogger 호출 제거, 파일 상단 logger 재사용
    const convertedMessages = messages.map(m => {
      if (m.role === 'tool') {
        return { ...m, role: 'user' as const };
      }
      return m;
    });

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
}
```

## 예상 효과

1. **코드 라인 수 감소**: `generateDeterministicToolCallId` 메서드가 20줄에서 3줄로 단축
2. **유지보수성 향상**: 외부 라이브러리 사용으로 버그 위험 감소
3. **일관성 향상**: 단일 로거 컨텍스트로 디버깅 용이성 증대
4. **안정성 향상**: 안전한 JSON 파싱으로 런타임 에러 방지
5. **가독성 향상**: 타입 분리로 메서드 구조 명확화

## 구현 순서

1. `tryParse` 유틸리티 함수 추가
2. `GeminiServiceConfig` 인터페이스 파일 상단 이동
3. 통일된 모듈 로거 적용
4. `generateDeterministicToolCallId` → `generateToolCallId`로 단순화
5. 안전한 JSON 파싱 적용
6. 테스트 실행 및 검증
