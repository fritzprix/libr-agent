# MCP 타입 시스템 리팩터링 계획

## 🎯 목표

MCP tool 호출 결과의 타입 불일치 문제를 해결하고, 에러 처리의 일관성을 확보하여 `success: true`인데 실제로는 에러인 상황을 방지합니다.

## 🚨 현재 문제점

### 1. 타입 일관성 문제

- **MCPResponse** (JSON-RPC 2.0 준수) - 표준 타입
- **ToolCallResult** (Rust backend용) - 구조가 다름 (`success: boolean`)
- **LegacyToolCallResult** (레거시용) - 더 혼란스러운 구조 (`success`, `isError` 중복)

### 2. Helper 함수명 불일치

```typescript
// 중복된 함수들
isSuccessResponse() vs isMCPSuccess()
isErrorResponse() vs isMCPError()
```

### 3. 에러 감지 로직 결함

- `success: true`인데 `result`에 에러 메시지가 있는 경우를 감지하지 못함
- `normalizeLegacyResponse` 함수가 에러 패턴을 제대로 인식하지 못함

### 4. MCPResult 타입의 모호함

- `content`와 `structuredContent` 모두 optional → 빈 객체도 유효한 결과가 됨

## 📋 수정 계획

### Phase 1: 타입 정의 개선 (`src/lib/mcp-types.ts`)

#### 1.1 타입 통합 및 단순화

```typescript
// ✅ 유지: MCPResponse (표준)
export interface MCPResponse {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult;
  error?: MCPError;
}

// ❌ 제거: ToolCallResult, LegacyToolCallResult
// → 모든 곳에서 MCPResponse만 사용
```

#### 1.2 MCPResult 타입 개선

```typescript
export interface MCPResult {
  content?: MCPContent[];
  structuredContent?: Record<string, unknown>;
}

// 타입 가드 추가
export function isValidMCPResult(result: MCPResult): boolean {
  return !!(result.content?.length || result.structuredContent);
}
```

#### 1.3 Helper 함수 정리

```typescript
// ✅ 유지 & 개선: 타입 가드 기능 추가
export function isMCPSuccess(
  response: MCPResponse,
): response is MCPResponse & { result: MCPResult } {
  return response.error === undefined && response.result !== undefined;
}

export function isMCPError(
  response: MCPResponse,
): response is MCPResponse & { error: MCPError } {
  return response.error !== undefined;
}

// ❌ 제거: isSuccessResponse, isErrorResponse
```

#### 1.4 강화된 변환 함수

```typescript
export function normalizeToolResult(
  result: unknown,
  toolName: string,
): MCPResponse {
  const id = `tool-${toolName}-${Date.now()}`;

  // 이미 MCPResponse인 경우
  if (typeof result === 'object' && result !== null && 'jsonrpc' in result) {
    return result as MCPResponse;
  }

  // 🔍 에러 패턴 감지 (핵심 개선사항)
  const isError =
    (typeof result === 'string' && result.includes('error')) ||
    (typeof result === 'object' && result !== null && 'error' in result) ||
    (typeof result === 'object' &&
      result !== null &&
      'success' in result &&
      !(result as any).success);

  if (isError) {
    const errorMessage =
      typeof result === 'string'
        ? result
        : typeof result === 'object' && result !== null && 'error' in result
          ? (result as any).error
          : 'Unknown error';

    return {
      jsonrpc: '2.0',
      id,
      error: {
        code: -32603,
        message: errorMessage,
        data: result,
      },
    };
  }

  // 성공 케이스
  return {
    jsonrpc: '2.0',
    id,
    result: {
      content: [
        {
          type: 'text',
          text: typeof result === 'string' ? result : JSON.stringify(result),
        },
      ],
    },
  };
}
```

### Phase 2: Rust 백엔드 수정 (`src-tauri/src/`)

#### 2.1 ToolCallResult 타입 제거

```rust
// ❌ 제거
pub struct ToolCallResult {
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

// ✅ 대체: MCPResponse 구조 사용
#[derive(Debug, Serialize, Deserialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<MCPError>,
}
```

#### 2.2 Tauri 명령어 수정

```rust
// src-tauri/src/lib.rs
#[tauri::command]
async fn call_mcp_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {  // ✅ 반환 타입 변경
    // 구현 수정
}
```

#### 2.3 MCP 호출 로직 수정

```rust
// src-tauri/src/mcp.rs
impl MCPServerManager {
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResponse {  // ✅ 반환 타입 변경
        // JSON-RPC 2.0 형식으로 응답 생성
    }
}
```

### Phase 3: 프론트엔드 수정

#### 3.1 ToolCaller.tsx 수정

```typescript
// src/features/chat/orchestrators/ToolCaller.tsx

import {
  MCPResponse,
  isMCPSuccess,
  isMCPError,
  normalizeToolResult,  // ✅ 새로운 함수 사용
  mcpResponseToString
} from '@/lib/mcp-types';

// MCP 호출 결과 처리
const mcpResponse: MCPResponse = await callMcpTool(...);

// ❌ 기존 normalizedResult 로직 제거
// ✅ MCPResponse 직접 사용
if (isMCPSuccess(mcpResponse)) {
  // 성공 처리
  const content = mcpResponseToString(mcpResponse);
} else if (isMCPError(mcpResponse)) {
  // 에러 처리
  const errorMessage = mcpResponse.error.message;
}
```

#### 3.2 MCP 서비스 레이어 수정

```typescript
// src/hooks/use-mcp-server.ts
// src/lib/tauri-mcp-client.ts

// 모든 MCP 관련 함수가 MCPResponse를 반환하도록 수정
export async function executeToolCall(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResponse> {
  // ✅ 반환 타입 통일
  // 구현
}
```

### Phase 4: 로깅 및 직렬화 수정

#### 4.1 SerializedToolResult 개선

```typescript
// src/features/chat/orchestrators/ToolCaller.tsx

interface SerializedToolResult {
  success: boolean;
  content?: string;
  error?: string;
  metadata: Record<string, unknown>;
  toolName: string;
  executionTime: number;
  timestamp: string;
}

const serializeToolResult = useCallback(
  (
    mcpResponse: MCPResponse,
    toolName: string,
    executionStartTime: number,
  ): string => {
    const result: SerializedToolResult = {
      success: isMCPSuccess(mcpResponse), // ✅ 정확한 성공/실패 판별
      content: isMCPSuccess(mcpResponse)
        ? mcpResponseToString(mcpResponse)
        : undefined,
      error: isMCPError(mcpResponse) ? mcpResponse.error.message : undefined,
      metadata: {
        toolName,
        mcpResponseId: mcpResponse.id,
        jsonrpc: mcpResponse.jsonrpc,
      },
      toolName,
      executionTime: Date.now() - executionStartTime,
      timestamp: new Date().toISOString(),
    };

    return JSON.stringify(result);
  },
  [],
);
```

## 🔄 마이그레이션 단계

### Step 1: 타입 정의 수정

1. `src/lib/mcp-types.ts` 수정
2. 새로운 helper 함수 추가
3. 레거시 타입/함수 deprecated 마킹

### Step 2: Rust 백엔드 수정

1. `src-tauri/src/mcp.rs` 타입 수정
2. `src-tauri/src/lib.rs` Tauri 명령어 수정
3. 테스트 및 검증

### Step 3: 프론트엔드 수정

1. `ToolCaller.tsx` 수정
2. MCP 관련 훅/서비스 수정
3. 에러 처리 로직 개선

### Step 4: 정리 및 검증

1. 레거시 타입/함수 완전 제거
2. 전체 시스템 테스트
3. 문서 업데이트

## ✅ 기대 효과

1. **에러 처리 일관성**: `success: true`인데 실제로는 에러인 상황 방지
2. **타입 안전성**: 컴파일 타임에 타입 오류 감지
3. **코드 단순화**: 타입 변환 로직 최소화
4. **유지보수성**: 하나의 표준 타입으로 통일
5. **확장성**: 새로운 MCP 서버/툴 추가 시 일관된 처리

## 🚨 주의사항

1. **점진적 마이그레이션**: 기존 코드 동작에 영향을 주지 않도록 단계적 적용
2. **테스트 강화**: 각 단계마다 철저한 테스트 수행
3. **문서 동기화**: 타입 변경사항을 문서에 반영
4. **팀 공유**: 새로운 타입 사용법을 팀원들과 공유
