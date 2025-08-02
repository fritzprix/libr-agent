# SynapticFlow - Gemini Tool Calling Error 해결 및 개선 사항

## 🔍 문제 분석

### 발견된 주요 이슈

1. **Gemini API 대화 턴 순서 위반**
   - 에러 메시지: "Please ensure that function call turn comes immediately after a user turn or after a function response turn."
   - 긴 대화(20개 메시지)에서 복잡한 턴 구조로 인한 문제
   - 연속적인 함수 호출로 인한 프로토콜 위반

2. **도구 호출 실패 시 부적절한 처리**
   - 📍 **위치**: `src/features/chat/orchestrators/ToolCaller.tsx`
   - 현재 코드에서 에러 처리 로직 완전 부재
   - Exception throw 시만 catch되고, `result.error !== undefined` 케이스 미처리
   - 실패 시 사용자에게 피드백 없음
   - 대화 흐름 중단 가능성
   - AI의 self-reflection 기능 없음

3. **타입 정의 불일치 및 MCP 프로토콜 위반**
   - 📍 **위치**: `src/lib/tauri-mcp-client.ts` vs `src/context/LocalToolContext.tsx`
   - `ToolCallResult`와 `MCPResponse` 인터페이스 불일치
   - MCP 프로토콜 표준 미준수 (JSON-RPC 2.0 구조 부재)
   - 📍 **위치**: `src/context/MCPServerContext.tsx`의 `executeToolCall` 함수
   - `result.error === true` 또는 `result.isError === true` 상황 감지 로직 부족
   - 📍 **위치**: `src/lib/ai-service/validators.ts`
   - MCP 응답 검증 및 정규화 로직 부재

## 🛠️ 개선 방안

### 1. ToolCaller 로직 개선

**현재 문제:**

- 도구 호출 실패 시 에러 처리 없음
- `result.error !== undefined` 케이스 미처리 (중요!)
- 실패한 경우에도 무조건 recursive submit
- AI의 self-reflection 기능 없음

**개선된 구현:**

```typescript
interface ToolExecutionResult {
  success: boolean;
  toolName: string;
  result?: unknown;
  error?: string;
  errorType?: 'exception' | 'result_error';
}

const execute = useAsyncFn(
  async (tcMessage: Message) => {
    const toolResults: Message[] = [];
    const executionResults: ToolExecutionResult[] = [];

    // 각 도구 호출을 순차적으로 실행하며 모든 에러 케이스 처리
    for (const toolCall of tcMessage.tool_calls!) {
      const toolName = toolCall.function.name;
      let executionResult: ToolExecutionResult;

      try {
        const callFunction = isLocalTool(toolName) ? callLocalTool : callMcpTool;
        const result = await callFunction(toolCall);

        // 🔍 중요: result.error 체크 (MCP 프로토콜의 에러 응답)
        if (result.error !== undefined || result.isError === true) {
          executionResult = {
            success: false,
            toolName,
            error: typeof result.error === 'string' 
              ? result.error 
              : JSON.stringify(result.error),
            errorType: 'result_error',
            result
          };

          // 에러 상태의 tool 메시지 생성
          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool',
            content: JSON.stringify({
              error: executionResult.error,
              tool_name: toolName,
              status: 'failed',
              type: 'result_error'
            }),
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });

          logger.warn(`Tool call failed with result error: ${toolName}`, { 
            toolCall, 
            result,
            error: executionResult.error 
          });

        } else {
          // 성공한 경우
          executionResult = {
            success: true,
            toolName,
            result
          };

          toolResults.push({
            id: createId(),
            assistantId: currentAssistant?.id,
            role: 'tool',
            content: result.content,
            tool_call_id: toolCall.id,
            sessionId: currentSession?.id || '',
          });

          logger.info(`Tool call successful: ${toolName}`, { toolCall, result });
        }

      } catch (error: unknown) {
        // Exception이 발생한 경우
        const errorMessage = error instanceof Error ? error.message : String(error);
        
        executionResult = {
          success: false,
          toolName,
          error: errorMessage,
          errorType: 'exception'
        };

        // Exception 에러 상태의 tool 메시지 생성
        toolResults.push({
          id: createId(),
          assistantId: currentAssistant?.id,
          role: 'tool',
          content: JSON.stringify({
            error: errorMessage,
            tool_name: toolName,
            status: 'failed',
            type: 'exception'
          }),
          tool_call_id: toolCall.id,
          sessionId: currentSession?.id || '',
        });

        logger.error(`Tool call failed with exception: ${toolName}`, { 
          toolCall, 
          error: errorMessage 
        });
      }

      executionResults.push(executionResult);
    }

    // 1단계: 모든 도구 호출 결과를 먼저 제출
    if (toolResults.length > 0) {
      await submit(toolResults);
    }

    // 2단계: 실패한 호출이 있는 경우 AI의 reflection 메시지 생성
    const failedResults = executionResults.filter(r => !r.success);
    if (failedResults.length > 0) {
      const reflectionMessage = generateReflectionMessage(failedResults, executionResults);
      await submit([reflectionMessage]);
    }
  },
  [submit, callLocalTool, callMcpTool, currentAssistant, currentSession],
);

// AI의 self-reflection을 위한 메시지 생성
const generateReflectionMessage = (
  failedResults: ToolExecutionResult[],
  allResults: ToolExecutionResult[]
): Message => {
  const totalCalls = allResults.length;
  const successfulCalls = totalCalls - failedResults.length;

  let reflectionContent = `🤔 도구 호출 결과를 분석해보겠습니다.\n\n`;
  
  if (successfulCalls > 0) {
    reflectionContent += `✅ 성공: ${successfulCalls}/${totalCalls}개 도구가 정상 작동했습니다.\n`;
  }

  if (failedResults.length > 0) {
    reflectionContent += `❌ 실패: ${failedResults.length}개 도구에서 문제가 발생했습니다.\n\n`;
    
    reflectionContent += `실패한 도구들을 분석해보겠습니다:\n`;
    failedResults.forEach(({ toolName, error, errorType }, index) => {
      reflectionContent += `${index + 1}. **${toolName}** (${errorType}): ${error}\n`;
    });

    reflectionContent += `\n이러한 문제들을 해결하기 위해 다음과 같은 접근을 시도해보겠습니다:\n`;
    reflectionContent += `• 다른 도구나 방법으로 같은 작업 수행\n`;
    reflectionContent += `• 입력 매개변수를 조정하여 재시도\n`;
    reflectionContent += `• 작업을 더 작은 단위로 분할하여 진행\n`;
    reflectionContent += `• 사용자에게 추가 정보를 요청\n\n`;
    reflectionContent += `어떤 방법을 선호하시나요?`;
  }

  return {
    id: createId(),
    assistantId: currentAssistant?.id,
    role: 'assistant',
    content: reflectionContent,
    sessionId: currentSession?.id || '',
  };
};
```

**주요 개선점:**

- 🎯 **Exception과 result.error 두 가지 실패 케이스 모두 처리**
- 🤖 **AI의 self-reflection 메시지로 문제 분석 및 대안 제시**
- 📊 **상세한 실패 통계와 원인 분석 제공**
- 🔄 **대화 흐름 유지하면서 자연스러운 에러 복구**
- 📝 **구체적인 에러 타입 구분 (exception vs result_error)**

### 2. 타입 시스템 통합 및 표준화

**현재 문제:**

- 📍 **위치**: `src/lib/tauri-mcp-client.ts` (줄 200)
- 현재 `ToolCallResult` 인터페이스가 단순함: `{ success: boolean; result?: unknown; error?: string; }`
- 📍 **위치**: `src/context/LocalToolContext.tsx` (줄 45-55)
- `MCPResponse`는 JSON-RPC 2.0 표준을 따름: `{ jsonrpc: '2.0'; id: number | string; success: boolean; ... }`
- 두 타입이 서로 다른 구조로 인한 혼란
- 📍 **위치**: `src/context/MCPServerContext.tsx`의 `executeToolCall` 함수 (줄 120-150)
- 원시 결과에 대한 검증 없이 바로 사용
- 📍 **위치**: `src/lib/ai-service/validators.ts`
- MCP 응답 처리 로직 부재

**통합된 타입 정의 (📍 수정 위치: `src/lib/tauri-mcp-client.ts`):**

```typescript
// MCP 프로토콜 표준을 따르는 응답 인터페이스
export interface MCPResponse {
  jsonrpc: '2.0';
  id: number | string;
  success: boolean;
  result?: MCPResult;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

// 하위 호환성을 위한 별칭 (기존 ToolCallResult 대체)
export type ToolCallResult = MCPResponse;
```

**개선된 검증 및 변환 로직 (📍 수정 위치: `src/lib/ai-service/validators.ts`):**

```typescript
export class MessageValidator {
  // 📍 새로 추가할 메서드
  static validateAndNormalizeMCPResponse(
    response: unknown,
    toolName: string,
  ): MCPResponse {
    // 레거시 응답을 MCP 형태로 변환
    // result.error === true 또는 result.isError === true 감지
    // 표준 JSON-RPC 2.0 형식 보장
    if (!response || typeof response !== 'object') {
      return {
        jsonrpc: '2.0',
        id: 'unknown',
        success: false,
        error: { code: -32602, message: `Invalid response from ${toolName}` }
      };
    }

    const responseObj = response as Record<string, unknown>;
    
    // 에러 상황 감지 (핵심 로직!)
    if (responseObj.error !== undefined || responseObj.isError === true) {
      return {
        jsonrpc: '2.0',
        id: responseObj.id || 'unknown',
        success: false,
        error: {
          code: -32603,
          message: typeof responseObj.error === 'string' 
            ? responseObj.error 
            : `Tool execution failed: ${toolName}`,
          data: responseObj.error
        }
      };
    }
    
    // 성공 케이스 처리...
  }

  // 📍 새로 추가할 메서드  
  static formatMCPResponseForChat(
    mcpResponse: MCPResponse,
    toolCallId: string,
  ): ToolCallResult {
    // MCP 응답을 Chat 시스템용으로 안전하게 변환
    // 다양한 Content 타입 지원 (text, image, resource 등)
    // 에러 상황 명확히 표시
  }
}
```

### 3. MCPServerContext 개선

**현재 문제:**

- 📍 **위치**: `src/context/MCPServerContext.tsx` (줄 120-150의 `executeToolCall` 함수)
- `rawResult`를 바로 사용하여 `result.error === true` 상황 미감지
- 원시 결과에 대한 타입 검증 로직 없음
- 일관되지 않은 에러 응답 형식

**개선된 executeToolCall (📍 수정 위치: `src/context/MCPServerContext.tsx`):**

```typescript
const executeToolCall = useCallback(
  async (toolCall) => {
    // 기존 매개변수 검증 로직...
    
    try {
      // 1단계: 원시 결과 가져오기
      const rawResult = await tauriMCPClient.callTool(
        serverName,
        toolName,
        toolArguments,
      );
      
      logger.debug(`Raw tool execution result for ${toolCall.function.name}:`, {
        rawResult,
      });

      // 2단계: 📍 핵심 개선 - 결과를 검증하고 정규화
      const validatedResult = MessageValidator.validateAndNormalizeMCPResponse(
        rawResult, 
        aiProvidedToolName
      );

      // 3단계: Chat 시스템용으로 포맷 변환
      const formattedResult = MessageValidator.formatMCPResponseForChat(
        validatedResult, 
        toolCall.id
      );

      // 4단계: 로깅 (성공/실패 구분)
      if (formattedResult.isError) {
        logger.warn(`Tool execution completed with error for ${toolCall.function.name}:`, {
          result: formattedResult,
        });
      } else {
        logger.debug(`Tool execution successful for ${toolCall.function.name}:`, {
          result: formattedResult,
        });
      }

      return formattedResult;

    } catch (execError) {
      // 5단계: Exception 처리 - 표준화된 에러 응답
      logger.error(`Tool execution failed for ${toolCall.function.name}:`, {
        execError,
      });
      
      return {
        role: 'tool',
        content: JSON.stringify({
          error: `Tool '${toolCall.function.name}' failed: ${execError instanceof Error ? execError.message : String(execError)}`,
          success: false
        }),
        tool_call_id: toolCall.id,
        isError: true,
      };
    }
  },
  [],
);
```

## 🎯 기대 효과

### 1. 안정성 향상

- 도구 호출 실패 시에도 안정적인 대화 흐름 유지
- 표준화된 에러 처리로 예측 가능한 동작

### 2. 사용자 경험 개선

- 실패 원인에 대한 명확한 피드백
- AI가 문제를 분석하고 대안을 제시
- 대화 중단 없이 자연스러운 흐름 유지

### 3. 개발자 경험 향상

- 일관된 타입 시스템으로 개발 편의성 증대
- MCP 프로토콜 표준 준수로 상호 운용성 보장
- 명확한 에러 추적 및 디버깅 가능

### 4. 유지보수성 향상

- 중앙집중화된 타입 정의
- 표준화된 검증 및 변환 로직
- 일관된 에러 처리 패턴

## 📋 구현 우선순위

### Phase 1: 긴급 수정 (🚨 즉시 적용 필요)

1. 📍 **`src/features/chat/orchestrators/ToolCaller.tsx` 에러 처리 로직 추가**
   - `result.error !== undefined` 체크 로직 추가
   - AI reflection 메시지 생성 함수 구현
   - Exception과 result_error 구분 처리

2. 📍 **`src/context/MCPServerContext.tsx` 기본 검증 로직 추가**
   - `rawResult.error` 및 `rawResult.isError` 체크
   - 기본적인 에러 응답 표준화

### Phase 2: 타입 시스템 통합 (🔧 체계적 리팩토링)

1. 📍 **`src/lib/tauri-mcp-client.ts` 타입 정의 통합**
   - 기존 `ToolCallResult` → `MCPResponse` 타입으로 교체
   - 하위 호환성을 위한 타입 별칭 유지

2. 📍 **`src/lib/ai-service/validators.ts`에 MCP 응답 검증 로직 추가**
   - `validateAndNormalizeMCPResponse` 메서드 구현
   - `formatMCPResponseForChat` 메서드 구현

3. 📍 **`src/context/MCPServerContext.tsx` 전면 업데이트**
   - 새로운 검증 로직 적용
   - 일관된 에러 처리 패턴 적용

### Phase 3: 고도화

1. 고급 실패 분석 및 복구 로직
2. 성능 최적화 및 캐싱
3. 종합적인 테스트 커버리지

## 🔧 적용 방법

1. **즉시 적용 가능한 수정사항 (🚨 우선순위 1):**
   - 📍 **`ToolCaller.tsx`**: try-catch 블록에 `result.error` 체크 추가
   - 📍 **`MCPServerContext.tsx`**: 기본적인 `rawResult.error` 검증 추가
   - 기본적인 AI reflection 메시지 생성

2. **단계적 리팩토링 (🔧 우선순위 2):**
   - 📍 **타입 정의 통합**: 하위 호환성 유지하며 점진적 교체
   - 📍 **검증 로직 점진적 적용**: 새로운 코드부터 적용, 기존 코드는 순차적 마이그레이션
   - 📍 **테스트를 통한 안정성 확보**: 각 단계별 회귀 테스트 실시

3. **검증 및 모니터링 (📊 지속적 개선):**
   - 📍 **에러 로그 모니터링**: `src/lib/logger.ts`의 로그 레벨별 분석
   - 📍 **사용자 피드백 수집**: 도구 호출 실패 시 사용자 경험 개선 여부 확인
   - 📍 **성능 메트릭 추적**: 도구 호출 성공률, 응답 시간, 에러 복구율 측정

---

**최종 목표:** 안정적이고 사용자 친화적인 AI 도구 호출 시스템 구축
