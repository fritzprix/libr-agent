# MCP Error Handling Standardization Refactoring Plan

**Date**: 2025-11-23 17:30  
**Objective**: Align MCP tool error handling across TypeScript modules and Rust builtin servers with MCP protocol standards.

---

## 작업의 목적

MCP 프로토콜은 **Protocol Error**와 **Tool Execution Error**를 명확하게 구분합니다:

- **Protocol Error** (`MCPResponse.error`): JSON-RPC 전송 계층 에러 (서버 응답 없음, JSON 파싱 실패 등)
- **Tool Execution Error** (`MCPResult.isError: true`): 도구 로직 실행 실패 (파일 없음, 권한 부족, 잘못된 파라미터 등)

현재 시스템의 많은 도구들이 로직 에러를 Protocol Error로 반환하고 있어, UI에서 이를 제대로 표시하지 못하고 있습니다. 이번 리팩토링의 목표는:

1. **TypeScript 모듈**: `throw new Error(...)`를 `createMCPErrorToolResult(...)`로 변경
2. **Rust Builtin 도구**: `Err(String)`을 `Ok(MCPResult::error(...))`로 변경
3. **Frontend**: `MCPResult.isError`를 `Message.error`로 매핑 (이미 완료됨 - `use-tool-processor.ts`)

이를 통해 사용자는 에러가 발생한 도구 호출을 빨간 배지로 명확히 인식할 수 있게 됩니다.

---

## 현재의 상태 / 문제점

### A. 아키텍처 현황

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend (React)                      │
├─────────────────────────────────────────────────────────────┤
│  use-tool-processor.ts                                       │
│  ├─ executeToolCall() → MCPResponse<MCPResult>              │
│  ├─ Maps isMCPError() → Message.error (Protocol Error)      │
│  └─ Maps MCPResult.isError → Message.error (Tool Error) ✅  │
│                                                               │
│  ToolCallResultBubble.tsx                                    │
│  └─ hasToolCallError() checks Message.error                 │
│     → Red badge for errors ✅                                │
└─────────────────────────────────────────────────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                  Service Layer (TS/Rust)                     │
├─────────────────────────────────────────────────────────────┤
│  TypeScript Web MCP Modules (src/lib/web-mcp/modules)       │
│  ├─ assistant-manager                                        │
│  ├─ bootstrap-server                                         │
│  ├─ mcp-manager                                              │
│  ├─ planning-server                                          │
│  └─ playbook-store                                           │
│     ❌ Problem: throw new Error(...) → Protocol Error       │
│                                                               │
│  Rust Builtin Servers (src-tauri/src/mcp/builtin)           │
│  ├─ workspace (file_operations, code_execution, etc.)       │
│  └─ content_store (handlers)                                 │
│     ❌ Problem: Err(String) → Registry wraps as Protocol    │
│                  Error with code -32603                      │
└─────────────────────────────────────────────────────────────┘
```

### B. 구체적 문제 코드

#### TypeScript 모듈 (Web MCP)

**위치**: `src/lib/web-mcp/modules/assistant-manager/server.ts`

```typescript
case 'get_assistant': {
  const { id } = typedArgs;
  if (!id) throw new Error('ID is required'); // ❌ Protocol Error로 변환됨
  // ...
}
```

**현재 동작**: `throw`가 발생하면 Promise rejection → 상위에서 `MCPResponse.error`로 감싸짐 → 프로토콜 에러로 처리

**예상되는 동작**: `return createMCPErrorToolResult('ID is required')` → `MCPResult.isError: true` → 프론트엔드에서 Tool Error로 매핑

#### Rust Builtin 도구

**위치**: `src-tauri/src/mcp/builtin/workspace/file_operations.rs`

```rust
pub async fn handle_read_file(&self, args: Value) -> Result<MCPResult, String> {
    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(path) => path,
        None => {
            return Err("Missing required parameter: path".to_string()); // ❌
        }
    };
    // ...
}
```

**현재 동작**: `Err(String)` 반환 → `BuiltinServerRegistry::call_tool`에서 JSON-RPC Error로 감싸짐 (`code: -32603`)

**예상되는 동작**: `Ok(MCPResult::error("Missing required parameter: path"))` → `isError: true` → 프론트엔드에서 Tool Error로 매핑

---

## 관련 코드의 구조 및 동작 방식 Summary

### 1. Error Response Flow (Birdeye View)

```
┌──────────────────────────────────────────────────────────────────┐
│  Tool Execution                                                   │
├──────────────────────────────────────────────────────────────────┤
│  TypeScript Module                  Rust Builtin                 │
│  ┌──────────────────┐              ┌──────────────────┐          │
│  │ callTool()       │              │ call_tool()      │          │
│  │                  │              │                  │          │
│  │ ❌ throw Error   │              │ ❌ Err(String)   │          │
│  │                  │              │                  │          │
│  │ ✅ return        │              │ ✅ Ok(MCPResult  │          │
│  │ MCPErrorToolResult│              │   ::error())     │          │
│  └────────┬─────────┘              └────────┬─────────┘          │
│           │                                 │                     │
│           ▼                                 ▼                     │
│  ┌─────────────────────────────────────────────────────┐         │
│  │  Registry / Wrapper Layer                            │         │
│  │  - BuiltinServerRegistry (Rust)                      │         │
│  │  - Web MCP Worker Proxy (TS)                         │         │
│  │                                                       │         │
│  │  Wraps into MCPResponse:                             │         │
│  │  { jsonrpc: "2.0", result: MCPResult, error?: ... }  │         │
│  └───────────────────────┬──────────────────────────────┘         │
│                          ▼                                         │
│  ┌──────────────────────────────────────────────────────┐         │
│  │  Frontend: use-tool-processor.ts                      │         │
│  │                                                        │         │
│  │  1. Check isMCPError(response) → Message.error        │         │
│  │  2. Check response.result?.isError → Message.error ✅ │         │
│  └───────────────────────┬──────────────────────────────┘         │
│                          ▼                                         │
│  ┌──────────────────────────────────────────────────────┐         │
│  │  UI: ToolCallResultBubble.tsx                         │         │
│  │                                                        │         │
│  │  hasToolCallError(Message) → Red badge                │         │
│  └──────────────────────────────────────────────────────┘         │
└──────────────────────────────────────────────────────────────────┘
```

### 2. Key Interfaces

**TypeScript (`src/lib/mcp/protocol/response.ts`)**:

```typescript
export interface MCPResult<T = unknown> {
  content?: MCPContent[];
  structuredContent?: T;
  isError?: boolean; // ← Tool Execution Error flag
}

export interface MCPResponse<T> {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult<T>;
  error?: MCPError; // ← Protocol Error
}
```

**Rust (`src-tauri/src/mcp/types.rs`)**:

```rust
pub struct MCPResult {
    pub content: Option<Vec<MCPContent>>,
    pub structured_content: Option<serde_json::Value>,
    pub is_error: Option<bool>, // ← Tool Execution Error flag
}

impl MCPResult {
    pub fn error(message: &str) -> Self { ... } // ✅ Already exists!
}
```

**Frontend (`src/models/chat.ts`)**:

```typescript
export interface Message {
  // ...
  error?: {
    displayMessage: string;
    type: MessageErrorType;
    recoverable: boolean;
    details?: { ... };
  };
}
```

### 3. Helper Functions

**TypeScript**: `src/lib/mcp-response-utils.ts`

```typescript
// ✅ Already available
export function createMCPErrorToolResult(
  message: string,
  data?: unknown,
): MCPResult<unknown> {
  return {
    content: [{ type: 'text', text: message }],
    isError: true,
    structuredContent: data ? { error: data } : undefined,
  };
}
```

**Rust**: `src-tauri/src/mcp/types.rs`

```rust
impl MCPResult {
    // ✅ Already available
    pub fn error(message: &str) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: message.to_string(),
            }]),
            structured_content: None,
            is_error: Some(true),
        }
    }
}
```

---

## 변경 이후의 상태 / 해결 판정 기준

### Success Criteria

1. **TypeScript Modules**:
   - All `throw new Error(...)` in tool handlers replaced with `return createMCPErrorToolResult(...)`
   - No Protocol Errors generated for validation/business logic failures

2. **Rust Builtin Tools**:
   - All `Err(String)` returns changed to `Ok(MCPResult::error(...))`
   - Registry no longer generates `-32603` errors for tool logic failures

3. **Frontend**:
   - `use-tool-processor.ts` correctly maps `isError: true` to `Message.error` ✅ (Already done)
   - `ToolCallResultBubble` displays red badge for all Tool Execution Errors ✅ (Already works)

4. **Behavior Validation**:
   - Test: Call a tool with invalid parameters → See red error badge in UI
   - Test: Call `read_file` with missing path → See "Missing parameter" error in UI
   - Test: Call `get_assistant` with missing ID → See "ID is required" error in UI

---

## 수정이 필요한 코드 및 수정 부분의 코드 스니핏

### Phase 1: TypeScript Web MCP Modules

#### 1.1 assistant-manager/server.ts

**Current (Lines 74-86)**:

```typescript
case 'get_assistant': {
  const { id } = typedArgs;
  if (!id) throw new Error('ID is required');
  const assistant = await service.getById(id);
  if (!assistant) {
    return createMCPErrorToolResult(
      `Assistant with ID ${id} not found`,
    );
  }
  return createMCPStructuredToolResult(
    `Found assistant: ${assistant.name}`,
    assistant,
  );
}
```

**Fixed**:

```typescript
case 'get_assistant': {
  const { id } = typedArgs;
  if (!id) {
    return createMCPErrorToolResult('ID is required');
  }
  const assistant = await service.getById(id);
  if (!assistant) {
    return createMCPErrorToolResult(
      `Assistant with ID ${id} not found`,
    );
  }
  return createMCPStructuredToolResult(
    `Found assistant: ${assistant.name}`,
    assistant,
  );
}
```

**Apply same pattern to**:

- `create_assistant` (Lines 96-97)
- `update_assistant` (Lines 119)
- `delete_assistant` (Lines 141)
- `search_assistants` (Lines 151)

#### 1.2 planning-server/server.ts

**Current (Lines 312-327)**:

```typescript
if (typeof thought !== 'string') {
  throw new Error('Invalid thought: must be a string');
}
// ... more validations
```

**Fixed**:

```typescript
if (typeof thought !== 'string') {
  return createMCPErrorToolResult('Invalid thought: must be a string');
}
```

#### 1.3 mcp-manager/server.ts

**Strategy**: Validation helpers (`validateTransportConfig`) currently throw. Two options:

**Option A**: Wrap callTool in try-catch

```typescript
async callTool(name: string, args: unknown): Promise<MCPResult> {
  try {
    // existing logic
  } catch (error) {
    const msg = error instanceof Error ? error.message : 'Unknown error';
    return createMCPErrorToolResult(msg);
  }
}
```

**Option B**: Change helpers to return `Result<T, string>` pattern

```typescript
function validateTransportConfig(
  t: unknown,
): { ok: true; value: TransportConfig } | { ok: false; error: string } {
  // validation logic
}
```

**Recommendation**: Option A (simpler, less refactoring)

### Phase 2: Rust Builtin Tools

#### 2.1 workspace/file_operations.rs

**Current (Lines 25-30)**:

```rust
pub async fn handle_read_file(&self, args: Value) -> Result<MCPResult, String> {
    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(path) => path,
        None => {
            return Err("Missing required parameter: path".to_string());
        }
    };
```

**Fixed**:

```rust
pub async fn handle_read_file(&self, args: Value) -> Result<MCPResult, String> {
    let path_str = match args.get("path").and_then(|v| v.as_str()) {
        Some(path) => path,
        None => {
            return Ok(MCPResult::error("Missing required parameter: path"));
        }
    };
```

**Apply to**:

- `handle_write_file`
- `handle_list_directory`
- `handle_replace_lines_in_file`
- All validation and IO error cases

#### 2.2 content_store/handlers.rs

**Current (Lines 13-16)**:

```rust
let args: AddContentArgs = match serde_json::from_value(params) {
    Ok(args) => args,
    Err(e) => {
        return Err(format!("Invalid add_content parameters: {e}"));
    }
};
```

**Fixed**:

```rust
let args: AddContentArgs = match serde_json::from_value(params) {
    Ok(args) => args,
    Err(e) => {
        return Ok(MCPResult::error(&format!("Invalid add_content parameters: {e}")));
    }
};
```

**Apply to all handlers in**:

- `handle_add_content`
- `handle_list_content`
- `handle_read_content`
- `handle_keyword_similarity_search`
- `handle_delete_content`

#### 2.3 workspace/code_execution.rs

**Pattern**: Process execution errors

```rust
// Current
Err(format!("Failed to spawn process: {e}"))

// Fixed
Ok(MCPResult::error(&format!("Failed to spawn process: {e}")))
```

---

## 재사용 가능한 연관 코드

### TypeScript

**Import Required**:

```typescript
import { createMCPErrorToolResult } from '@/lib/mcp-response-utils';
```

**Pattern**:

```typescript
// Validation
if (!requiredParam) {
  return createMCPErrorToolResult('Parameter is required');
}

// Not found
if (!resource) {
  return createMCPErrorToolResult(`Resource ${id} not found`, { id });
}

// Business logic error
if (invalidCondition) {
  return createMCPErrorToolResult('Operation not allowed', { reason: '...' });
}
```

### Rust

**Helper Already Exists**:

```rust
// src-tauri/src/mcp/types.rs (Lines 385-393)
impl MCPResult {
    pub fn error(message: &str) -> Self {
        Self {
            content: Some(vec![MCPContent::Text {
                text: message.to_string(),
            }]),
            structured_content: None,
            is_error: Some(true),
        }
    }
}
```

**Pattern**:

```rust
// Validation
if required_param.is_none() {
    return Ok(MCPResult::error("Parameter is required"));
}

// File IO errors
.map_err(|e| format!("Failed to read file: {e}"))?
// becomes
match operation {
    Ok(result) => { /* handle success */ }
    Err(e) => return Ok(MCPResult::error(&format!("Failed: {e}"))),
}
```

---

## Test Code 추가 및 수정 필요 부분에 대한 가이드

### Unit Tests (TypeScript)

**Location**: `src/lib/web-mcp/modules/__tests__/`

**Test Cases**:

```typescript
describe('assistant-manager error handling', () => {
  it('should return Tool Error for missing ID', async () => {
    const result = await server.callTool('get_assistant', {});

    expect(result.isError).toBe(true);
    expect(result.content?.[0]).toMatchObject({
      type: 'text',
      text: expect.stringContaining('ID is required'),
    });
  });

  it('should return Tool Error for not found', async () => {
    const result = await server.callTool('get_assistant', { id: 'invalid' });

    expect(result.isError).toBe(true);
    expect(result.content?.[0]).toMatchObject({
      type: 'text',
      text: expect.stringContaining('not found'),
    });
  });
});
```

### Integration Tests (Rust)

**Location**: `src-tauri/src/mcp/builtin/tests/`

**Test Cases**:

```rust
#[tokio::test]
async fn test_read_file_missing_path_returns_tool_error() {
    let server = WorkspaceServer::new(session_manager);
    let args = serde_json::json!({});

    let result = server.handle_read_file(args).await;

    assert!(result.is_ok());
    let mcp_result = result.unwrap();
    assert_eq!(mcp_result.is_error, Some(true));

    let text = mcp_result.content.and_then(|c| c.first().cloned());
    assert!(matches!(text, Some(MCPContent::Text { text }) if text.contains("Missing")));
}
```

### E2E Tests (Frontend)

**Location**: `src/test/integration/tool-error-handling.test.ts`

**Test Flow**:

1. Create session
2. Call tool with invalid params via `useUnifiedMCP`
3. Verify `Message.error` is populated
4. Render `ToolCallResultBubble`
5. Assert red badge is visible

---

## 작업 순서 (Implementation Steps)

### Step 1: TypeScript Modules (2-3 hours)

1. **assistant-manager/server.ts**
   - Replace all `throw new Error` with `createMCPErrorToolResult`
   - Add unit tests

2. **planning-server/server.ts**
   - Replace validation throws
   - Add unit tests

3. **mcp-manager/server.ts**
   - Wrap `callTool` in try-catch
   - Convert exceptions to Tool Errors

4. **bootstrap-server/server.ts**
   - Review and fix if needed (mostly uses `createMCPErrorToolResult` already)

### Step 2: Rust Builtin Tools (3-4 hours)

1. **workspace/file_operations.rs**
   - Replace all `Err(...)` with `Ok(MCPResult::error(...))`
   - Focus on: `handle_read_file`, `handle_write_file`, `handle_list_directory`

2. **workspace/code_execution.rs**
   - Fix process spawn errors
   - Fix execution timeout errors

3. **content_store/handlers.rs**
   - Fix all handler validation errors
   - Fix storage operation errors

4. **Add Rust unit tests**
   - Test missing parameters return Tool Errors
   - Test file not found returns Tool Errors

### Step 3: Frontend Validation (1 hour)

1. **Manual Testing**
   - Call tools with invalid params
   - Verify red error badges appear
   - Check `Message.error` structure in DevTools

2. **Add E2E tests**
   - Tool error rendering test
   - Error recovery test

### Step 4: Documentation (30 min)

1. Update `docs/architecture/error-handling.md`
2. Add migration notes to CHANGELOG.md

---

## Clarification Q-list

### Q1: Error Message Localization

**Question**: Should error messages remain in English, or do we need i18n support?
**Context**: Current messages are hardcoded English strings.
**Impact**: If i18n needed, we need to add message keys and translation files.

### Q2: Structured Error Data

**Question**: Should we standardize the `structuredContent` format for errors?
**Example**:

```typescript
{
  error: {
    code: 'MISSING_PARAMETER',
    field: 'id',
    expected: 'string',
    received: undefined
  }
}
```

**Impact**: Would enable better error handling in UI (e.g., highlight specific form fields).

### Q3: Backward Compatibility

**Question**: Do we need to support old Protocol Error format temporarily?
**Context**: External clients might expect JSON-RPC errors for validation failures.
**Recommendation**: No - MCP spec is clear, and we control all clients.

### Q4: Error Logging

**Question**: Should Tool Errors be logged differently than Protocol Errors?
**Context**: Currently both go through same logging pipeline.
**Recommendation**: Add `isToolError: boolean` field to log context for filtering.

---

## 추가 분석 과제

### Task 1: Error Metrics

Analyze error distribution after migration:

- How many errors were Protocol Errors before?
- How many are now Tool Errors?
- Are there any remaining Protocol Errors that should be Tool Errors?

### Task 2: Error Recovery UX

Investigate UI improvements:

- Should Tool Errors have a "Retry" button?
- Should we show different icons for different error types?
- Should validation errors suggest fixes? (e.g., "Did you mean: `path: './file.txt'`?")

### Task 3: Rust Error Handling Patterns

Evaluate whether to introduce `Result<MCPResult, Never>` pattern:

```rust
pub async fn handle_read_file(&self, args: Value) -> MCPResult {
    // Can never fail at transport level
}
```

This would make the type system enforce Tool Error usage.

---

## References

- **MCP Specification**: https://modelcontextprotocol.io/
- **JSON-RPC 2.0 Spec**: https://www.jsonrpc.org/specification
- **Project Error Handling Docs**: `docs/architecture/error-handling.md`
- **Frontend Mapping Code**: `src/hooks/use-tool-processor.ts` (Lines 106-155)
