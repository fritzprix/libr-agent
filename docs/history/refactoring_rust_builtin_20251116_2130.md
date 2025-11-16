# Rust Built-in Tools MCPResult Architecture Refactoring

## 작업의 목적

Rust 기반 Built-in Tool 서버(`content_store`, `workspace` 등)가 불필요하게 JSON-RPC 2.0 전송 계층(`MCPResponse`)을 생성하는 문제를 해결한다. Rust handler는 순수한 Tool 실행 결과(`MCPResult`)만 반환하고, Tauri Command layer에서 전송 계층 래핑을 일괄 처리하도록 아키텍처를 개선한다.

### 배경

- Web MCP 리팩토링 (`refactoring_20251116_2114.md`)에서 발견된 아키텍처 패턴을 Rust builtin tools에도 적용
- 동일한 문제: 각 handler에서 `MCPResponse` 직접 생성, 중복 코드, 복잡도 증가
- Tauri Command layer를 통해 일관된 래핑 전략 수립
- Frontend에서 Web MCP와 Rust builtin tools로부터 동일한 구조의 `MCPResponse` 수신

### 핵심 문제

**불필요한 전송 계층 래핑 (Rust에서):**

```rust
// 현재: Rust handler가 직접 MCPResponse 생성 (불필요!)
// workspace/mod.rs:206
pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
  let request_id = Self::generate_request_id();  // ← handler가 내부에서 ID 생성
  // ... implementation
  return Self::success_response(request_id, message);  // ← MCPResponse 직접 생성
}

// Trait definition (builtin/mod.rs:66)
#[async_trait]
pub trait BuiltinMCPServer {
  async fn call_tool(&self, tool_name: &str, args: Value, request_id: Option<Value>)
    -> MCPResponse;  // ← Trait도 MCPResponse 반환
}
```

**개선 방향:**

```rust
// 1. Rust handler는 순수 결과만 반환
pub async fn handle_poll_process(&self, args: Value) -> Result<MCPResult, String> {
  // request_id 파라미터 제거, ID 생성 제거
  return Ok(MCPResult {
    content: vec![MCPContent { type_: "text", text: /* content */ }],
    structured_content: None,
    is_error: false,
  });
}

// 2. Trait도 MCPResult 반환으로 변경
#[async_trait]
pub trait BuiltinMCPServer {
  async fn call_tool(&self, tool_name: &str, args: Value)
    -> Result<MCPResult, String>;  // ✅ MCPResult 반환
}

// 3. BuiltinServerRegistry에서 전송 계층 래핑 (단일 지점)
// builtin/mod.rs:520
pub async fn call_tool(..., request_id: Option<Value>) -> MCPResponse {
  if let Some(server) = self.get_server(server_name) {
    let normalized_args = Self::normalize_json_args(args);

    // ✅ Trait method returns Result<MCPResult>
    match server.call_tool(tool_name, normalized_args).await {
      Ok(mcp_result) => {
        let id = request_id.unwrap_or_else(|| Value::String(uuid::Uuid::new_v4().to_string()));
        MCPResponse {
          jsonrpc: "2.0".to_string(),
          id: Some(id),
          result: Some(mcp_result),
          error: None,
        }
      }
      Err(e) => { /* error MCPResponse */ }
    }
  }
}
```

## 현재의 상태 / 문제점

### 문제 1: Rust에서도 MCPResponse 직접 생성

**관련 파일 (Rust):**

- `src-tauri/src/mcp/builtin/workspace/mod.rs` - Tool handler 함수들
- `src-tauri/src/mcp/builtin/content_store/handlers.rs` - Content Store handlers
- `src-tauri/src/mcp/builtin/mod.rs` - BuiltinMCPServer trait 정의
- 기타 builtin modules

**특징:**

- Handler 함수들이 `MCPResponse` 반환 (전송 계층)
- 각 handler에서 `Self::generate_request_id()` 내부 호출
- `BuiltinMCPServer` trait의 `call_tool` 메서드가 `MCPResponse` 반환
- 50+ 곳의 handler 함수에서 중복 래핑
- `request_id` 생성 로직이 handler마다 분산

**예시 (workspace/mod.rs:206):**

```rust
pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let request_id = Self::generate_request_id();  // ← 문제: 내부에서 ID 생성

    let process_id = match args.get("process_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return Self::error_response(  // ← MCPResponse 직접 생성
                request_id,
                -32602,
                "Missing required parameter: process_id",
            );
        }
    };
    // ... implementation
    Self::success_response(request_id, message)  // ← MCPResponse 반환
}

// Trait definition (builtin/mod.rs:66)
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,  // ← Trait에서 받지만 handler로 전달 안됨
    ) -> MCPResponse;  // ← 문제: 전송 계층 반환
}
```

**문제점:**

- Handler가 전송 계층(`MCPResponse`) 생성 책임
- `request_id` 관리가 handler 내부에 분산
- Trait 레벨과 Handler 레벨 간 책임 불명확

### 문제 2: Trait과 Registry 간 책임 모호

**파일:** `src-tauri/src/mcp/builtin/mod.rs`

```rust
// Trait definition (line 66)
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,
    ) -> MCPResponse;  // ❌ 전송 계층 반환
}

// Registry's call_tool (line 520)
pub async fn call_tool(
    &self,
    server_name: &str,
    tool_name: &str,
    args: Value,
    request_id: Option<Value>,
) -> MCPResponse {
    if let Some(server) = self.get_server(server_name) {
        let normalized_args = Self::normalize_json_args(args);
        server
            .call_tool(tool_name, normalized_args, request_id)
            .await  // ← 단순 중계, 래핑 책임이 trait 구현체에 있음
    } else {
        // Error response...
    }
}
```

**문제:**

- Registry가 `request_id`를 trait method에 넘기기만 함
- 전송 계층 래핑이 각 서버의 trait 구현체에 분산
- Registry는 단순 중계 역할만 수행 (가치 없음)
- Error handling 불일치 (각 handler가 다른 방식)

### 문제 3: 타입 불일치 및 불일치한 에러 처리

**프론트엔드 (TypeScript):**

```typescript
// src/lib/backend/builtin-tools.ts
export function callBuiltinTool(
  serverName: string,
  toolName: string,
  args: unknown,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_builtin_tool', {
    serverName,
    toolName,
    args,
    requestId: generateId(), // Frontend에서 request_id 생성
  });
}
```

**문제:**

- Frontend가 `request_id` 생성 (Tauri command parameter로 전달)
- Rust handler가 자체적으로도 `request_id` 생성 (중복 책임)
- Web MCP와의 아키텍처 불일치 (Web MCP는 Worker가 래핑)
- Trait level과 Handler level 간 `request_id` 관리 불일치

### 문제 4: Error Response 표준화 부족

**현재:**

```rust
// 에러 처리 패턴 불일치
Err("File not found".to_string())
Err(format!("Failed to read {}: {}", path, e))
json!({"error": "Invalid arguments"})
```

**개선 필요:**

- 일관된 에러 포맷 (MCPResult with isError flag)
- 구조화된 에러 정보 (error code, context 등)
- Frontend의 error handling 단순화

## 관련 코드의 구조 및 동작 방식 Summary (Birdeye View)

### 현재 데이터 플로우 (문제 상태)

```text
┌──────────────────────────────────────────────┐
│ Frontend (TypeScript)                        │
│                                              │
│ callBuiltinTool(serverName, toolName, args) │
│   ↓                                          │
│ safeInvoke('call_builtin_tool', {          │
│   serverName, toolName, args, requestId    │
│ })  ← Frontend가 requestId 생성             │
└────────────────────┬─────────────────────────┘
                     │ Tauri IPC
                     ▼
┌──────────────────────────────────────────────────────┐
│ Tauri Backend (Rust)                                 │
│                                                      │
│ #[tauri::command]                                   │
│ async fn call_builtin_tool(..., request_id)         │
│ ) -> MCPResponse  (단순 중계)                       │
│   ↓                                                 │
│ MCPServerManager::call_builtin_tool(request_id)    │
│   ↓                                                 │
│ BuiltinServerRegistry::call_tool(request_id)       │
│   ↓                                                 │
│ server.call_tool(request_id)  ← Trait method       │
│   ↓                                                 │
│ WorkspaceServer::call_tool impl                     │
│   match tool_name {                                │
│     "poll_process" →                               │
│       self.handle_poll_process(args)  ❌           │
│         ↓                                          │
│       pub async fn handle_poll_process(            │
│         &self, args: Value                         │
│       ) -> MCPResponse {  ← 전송 계층 반환!         │
│         let request_id = Self::generate_request_id(); ❌│
│         // ... implementation                      │
│         Self::success_response(request_id, msg)  ❌ │
│       }                                             │
│   }                                                 │
│                                                      │
│ ❌ 문제: Handler가 MCPResponse 직접 생성           │
│ ❌ 문제: request_id를 내부에서 생성                │
│ ❌ 문제: Trait의 request_id가 handler로 전달 안됨  │
└──────────────────────────────────────────────────────┘
```

### 목표 데이터 플로우 (개선 후)

```text
┌──────────────────────────────────────────────┐
│ Frontend (TypeScript) - 변경 없음            │
│                                              │
│ callBuiltinTool(serverName, toolName, args) │
│   ↓                                          │
│ safeInvoke('call_builtin_tool', {          │
│   serverName, toolName, args, requestId    │
│ })  ← Frontend가 requestId 생성             │
└────────────────────┬─────────────────────────┘
                     │ Tauri IPC
                     ▼
┌──────────────────────────────────────────────────────┐
│ Tauri Backend (Rust)                                 │
│                                                      │
│ #[tauri::command]                                   │
│ async fn call_builtin_tool(..., request_id)         │
│ ) -> MCPResponse  (단순 중계 유지)                  │
│   ↓                                                 │
│ MCPServerManager::call_builtin_tool(request_id)    │
│   ↓                                                 │
│ BuiltinServerRegistry::call_tool(request_id)  ✅   │
│   ↓                                                 │
│ let mcp_result = server.call_tool() ← Trait변경! ✅│
│   ↓                                                 │
│ WorkspaceServer::call_tool impl  ✅                │
│   match tool_name {                                │
│     "poll_process" →                               │
│       self.handle_poll_process(args)  ✅           │
│         ↓                                          │
│       pub async fn handle_poll_process(            │
│         &self, args: Value                         │
│       ) -> Result<MCPResult, String> {  ✅         │
│         // request_id 파라미터 없음! ✅            │
│         // ... implementation                      │
│         Ok(MCPResult {                  ✅         │
│           content: vec![...],                      │
│           is_error: false,                         │
│         })                                         │
│       }                                             │
│   }                                                 │
│   ↓                                                 │
│ ✅ Registry에서 전송 계층 래핑:                    │
│ MCPResponse {                                       │
│   jsonrpc: "2.0",                                  │
│   id: request_id,  ← Registry가 설정               │
│   result: Some(mcp_result),                        │
│   error: None,                                     │
│ }                                                   │
└──────────────────────────────────────────────────────┘
```

### 핵심 컴포넌트

#### 1. Rust Handler 구조 (현재)

**위치들:** `src-tauri/src/mcp/builtin/*/mod.rs`, `*.rs`

**일반 패턴:**

```rust
pub async fn handle_tool_name(
  params: /* specific params */,
  request_id: String,  // ← 문제
) -> Result<MCPResponse, String> {  // ← 문제
  // ... business logic

  Ok(MCPResponse {  // ← 중복
    jsonrpc: "2.0".to_string(),  // ← 중복
    id: request_id,  // ← handler 책임
    result: Some(result),
    error: None,
  })
}
```

**영향 범위:**

- **Workspace tools:** 20-30개 handlers (file ops, terminal, code exec)
- **Content Store:** 10-15개 handlers (search, CRUD)
- **기타 builtin:** 5-10개 handlers

#### 2. Tauri Command Layer (현재)

**파일:** `src-tauri/src/commands/mcp_commands.rs`

```rust
#[tauri::command]
pub async fn call_builtin_tool(
  server_name: String,
  tool_name: String,
  args: serde_json::Value,
  request_id: String,
) -> Result<MCPResponse, String> {
  // 단순 중계
  match server_name.as_str() {
    "workspace" => {
      let args = parse_workspace_args(&tool_name, args)?;
      call_workspace_tool(&tool_name, args, request_id).await  // ← request_id 전달
    }
    "content_store" => {
      let args = parse_content_store_args(&tool_name, args)?;
      call_content_store_tool(&tool_name, args, request_id).await
    }
    _ => Err("Unknown server".to_string()),
  }
}
```

**문제:** Command가 `request_id`를 그냥 handler에 넘김

#### 3. Frontend Caller (TypeScript)

**파일:** `src/lib/backend/builtin-tools.ts`

```typescript
export function callBuiltinTool(
  serverName: string,
  toolName: string,
  args: unknown,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_builtin_tool', {
    serverName,
    toolName,
    args,
    requestId: generateId(), // Frontend가 생성
  });
}
```

#### 4. Type Definitions

**프론트엔드:** `src/lib/mcp/protocol/response.ts`

```typescript
export interface MCPResponse<T> {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult<T>;
  error?: MCPError;
}

export interface MCPResult<T = unknown> {
  content?: MCPContent[];
  structuredContent?: T;
  isError?: boolean;
}
```

**Rust 타입:** `src-tauri/src/mcp/types.rs` (추측)

```rust
#[derive(Serialize, Deserialize)]
pub struct MCPResponse {
  pub jsonrpc: String,
  pub id: String,  // 또는 serde_json::Value
  #[serde(skip_serializing_if = "Option::is_none")]
  pub result: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<MCPError>,
}

#[derive(Serialize, Deserialize)]
pub struct MCPResult {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<Vec<MCPContent>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub structured_content: Option<serde_json::Value>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub is_error: Option<bool>,
}
```

## 변경 이후의 상태 / 해결 판정 기준

### 목표 상태

#### 1. Handler 함수 시그니처 변경

```rust
// Before
pub async fn handle_read_file(
  workspace_path: &Path,
  args: serde_json::Value,
  request_id: String,  // ❌ 제거됨
) -> Result<MCPResponse, String> {  // ❌ MCPResult로 변경

// After
pub async fn handle_read_file(
  workspace_path: &Path,
  args: serde_json::Value,
) -> Result<MCPResult, String> {  // ✅
```

#### 2. Handler Return Value 변경

```rust
// Before
Ok(MCPResponse {
  jsonrpc: "2.0".to_string(),
  id: request_id,
  result: Some(result),
  error: None,
})

// After
Ok(MCPResult {
  content: Some(vec![MCPContent {
    type_: "text".to_string(),
    text: Some(content),
    ..Default::default()
  }]),
  structured_content: None,
  is_error: Some(false),
})
```

#### 3. Tauri Command 래핑 로직 추가

```rust
#[tauri::command]
pub async fn call_builtin_tool(
  server_name: String,
  tool_name: String,
  args: serde_json::Value,
  request_id: String,
) -> Result<MCPResponse, String> {
  let mcp_result = match server_name.as_str() {
    "workspace" => {
      let args = parse_workspace_args(&tool_name, args)?;
      call_workspace_tool(&tool_name, args).await?  // ✅ request_id 제거
    }
    "content_store" => {
      let args = parse_content_store_args(&tool_name, args)?;
      call_content_store_tool(&tool_name, args).await?
    }
    _ => return Err("Unknown server".to_string()),
  };

  // ✅ 래핑 로직 (이곳에 집중됨)
  Ok(MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: request_id,
    result: Some(mcp_result),
    error: None,
  })
}
```

### 성공 기준

#### 1. Handler 리팩토링 완료

- ✅ 50+ workspace handlers: `Result<MCPResult>` 반환
- ✅ 15+ content_store handlers: `Result<MCPResult>` 반환
- ✅ 모든 handlers: `request_id` 파라미터 제거
- ✅ 모든 handlers: MCPResponse 생성 코드 제거

#### 2. Tauri Command 래핑 로직 구현

- ✅ `call_builtin_tool` command: MCPResult를 MCPResponse로 래핑
- ✅ Error handling: MCPResult.isError 또는 MCPError 일관된 사용
- ✅ Type safety: Rust compile 성공

#### 3. 에러 처리 표준화

- ✅ 모든 handler error: 구조화된 MCPResult 또는 MCPError
- ✅ Frontend에서 일관된 error 처리 가능
- ✅ Error response에 context 정보 포함

#### 4. 테스트 및 검증

- ✅ Rust cargo test 성공
- ✅ Frontend 통합 테스트 통과
- ✅ E2E: builtin tool 실행 및 결과 처리
- ✅ `pnpm refactor:validate` 통과

## 수정이 필요한 코드 및 수정부분의 코드 스니핏

### 0. BuiltinMCPServer Trait 정의 변경 (최우선 작업)

**파일:** `src-tauri/src/mcp/builtin/mod.rs`

**이 변경이 모든 refactoring의 기반이 됩니다.**

```rust
// Before (현재)
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,  // ← Trait에서 받음
    ) -> MCPResponse;  // ❌ 전송 계층 반환
}

// After (목표)
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn tools(&self) -> Vec<MCPTool>;

    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        // request_id 파라미터 제거
    ) -> Result<MCPResult, String>;  // ✅ 순수 결과 반환
}
```

**영향 범위:**

- ✅ `WorkspaceServer` trait 구현 변경 필요
- ✅ `ContentStoreServer` trait 구현 변경 필요
- ✅ `BuiltinServerRegistry::call_tool()` 래핑 로직 추가 필요
- ✅ 모든 handler 함수 시그니처 변경

### 1. BuiltinServerRegistry 래핑 로직 추가

**파일:** `src-tauri/src/mcp/builtin/mod.rs` (line ~520)

**이곳이 전송 계층 래핑의 단일 지점이 됩니다.**

```rust
// Before (현재)
pub async fn call_tool(
    &self,
    server_name: &str,
    tool_name: &str,
    args: Value,
    request_id: Option<Value>,
) -> MCPResponse {
    if let Some(server) = self.get_server(server_name) {
        let normalized_args = Self::normalize_json_args(args);
        server
            .call_tool(tool_name, normalized_args, request_id)
            .await  // ← 단순 중계, 래핑은 server가 함
    } else {
        // Error response
        let request_id = request_id
            .unwrap_or_else(|| Value::String(uuid::Uuid::new_v4().to_string()));
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(request_id),
            result: None,
            error: Some(MCPError {
                code: -32601,
                message: format!("Built-in server '{server_name}' not found"),
                data: None,
            }),
        }
    }
}

// After (목표)
pub async fn call_tool(
    &self,
    server_name: &str,
    tool_name: &str,
    args: Value,
    request_id: Option<Value>,
) -> MCPResponse {
    // Generate request_id if not provided
    let id = request_id
        .unwrap_or_else(|| Value::String(uuid::Uuid::new_v4().to_string()));

    if let Some(server) = self.get_server(server_name) {
        let normalized_args = Self::normalize_json_args(args);

        // ✅ Trait method now returns Result<MCPResult, String>
        match server.call_tool(tool_name, normalized_args).await {
            Ok(mcp_result) => {
                // ✅ 성공 시 래핑 (단일 지점)
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: Some(serde_json::to_value(mcp_result).unwrap()),
                    error: None,
                }
            }
            Err(err_msg) => {
                // ✅ 에러 시 래핑 (단일 지점)
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,  // Internal error
                        message: err_msg,
                        data: None,
                    }),
                }
            }
        }
    } else {
        // Server not found error
        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: None,
            error: Some(MCPError {
                code: -32601,  // Method not found
                message: format!("Built-in server '{server_name}' not found"),
                data: None,
            }),
        }
    }
}
```

**장점:**

- 50+ handlers → 단일 래핑 포인트
- `request_id` 관리가 한 곳에 집중
- Error code 표준화 (-32601, -32603)
- 기존 상위 레이어(Tauri command, MCPServerManager) 변경 불필요

### 2. 타입 정의 추가 (Rust)

**파일:** `src-tauri/src/mcp/types.rs`

**추가 필요 (또는 기존 완성):**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResult {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<Vec<MCPContent>>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub structured_content: Option<serde_json::Value>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub is_error: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPContent {
  #[serde(rename = "type")]
  pub type_: String,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub text: Option<String>,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub resource: Option<serde_json::Value>,
}
```

### 3. Workspace Tools Handlers 리팩토링

**파일:** `src-tauri/src/mcp/builtin/workspace/mod.rs`

**변경 패턴 (모든 workspace handlers):**

```rust
// Before (현재 실제 코드)
pub async fn handle_poll_process(&self, args: Value) -> MCPResponse {
    let request_id = Self::generate_request_id();  // ← 내부에서 ID 생성

    // Parse process_id
    let process_id = match args.get("process_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            return Self::error_response(  // ← MCPResponse 생성
                request_id,
                -32602,
                "Missing required parameter: process_id",
            );
        }
    };

    // ... implementation

    Self::success_response(request_id, "Process polled successfully")
}

// After (목표)
pub async fn handle_poll_process(&self, args: Value) -> Result<MCPResult, String> {
    // request_id 파라미터 제거, 생성 로직 제거

    // Parse process_id with ? operator
    let process_id = args
        .get("process_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter: process_id".to_string())?;

    // ... implementation

    Ok(MCPResult {
        content: Some(vec![MCPContent {
            type_: "text".to_string(),
            text: Some("Process polled successfully".to_string()),
            resource: None,
        }]),
        structured_content: Some(json!({
            "process_id": process_id,
            "status": "running",
            // ... other metadata
        })),
        is_error: false,  // Remove Option
    })
}
```

**적용 대상 (약 20-25개 함수):**

- `handle_poll_process`
- `handle_read_process_output`
- `handle_list_processes`
- File operations handlers
- Code execution handlers
- Terminal management handlers

### 3. Content Store Handlers 리팩토링

**파일:** `src-tauri/src/mcp/builtin/content_store/handlers.rs`

**변경 패턴 (모든 content_store handlers):**

```rust
// Before
pub async fn handle_search(
  db: &ContentStoreDb,
  query: String,
  request_id: String,
) -> Result<MCPResponse, String> {
  let results = db.search(&query).await?;

  Ok(MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: request_id,
    result: Some(json!({
      "results": results,
      "count": results.len(),
    })),
    error: None,
  })
}

// After
pub async fn handle_search(
  db: &ContentStoreDb,
  query: String,
) -> Result<MCPResult, String> {
  let results = db.search(&query).await?;

  Ok(MCPResult {
    content: Some(vec![MCPContent {
      type_: "text".to_string(),
      text: Some(format!("Found {} items", results.len())),
      resource: None,
    }]),
    structured_content: Some(json!({
      "results": results,
      "count": results.len(),
    })),
    is_error: Some(false),
  })
}
```

**적용 대상 (약 12-15개 함수):**

- `handle_search`
- `handle_add_content`
- `handle_get_content`
- `handle_delete_content`
- `handle_list_sources`
- 기타 CRUD operations

### 5. WorkspaceServer Trait 구현 변경

**파일:** `src-tauri/src/mcp/builtin/workspace/mod.rs`

**Trait 구현 메서드 업데이트:**

```rust
// Before (현재 추정)
#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
        request_id: Option<Value>,
    ) -> MCPResponse {
        match tool_name {
            "poll_process" => self.handle_poll_process(args).await,
            "read_process_output" => self.handle_read_process_output(args).await,
            "list_processes" => self.handle_list_processes(args).await,
            // ... other tools
            _ => {
                let id = request_id.unwrap_or_else(||
                    Value::String(uuid::Uuid::new_v4().to_string())
                );
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: None,
                    error: Some(MCPError {
                        code: -32601,
                        message: format!("Unknown tool: {}", tool_name),
                        data: None,
                    }),
                }
            }
        }
    }
}

// After (목표)
#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    async fn call_tool(
        &self,
        tool_name: &str,
        args: Value,
    ) -> Result<MCPResult, String> {
        match tool_name {
            "poll_process" => self.handle_poll_process(args).await,
            "read_process_output" => self.handle_read_process_output(args).await,
            "list_processes" => self.handle_list_processes(args).await,
            // ... other tools
            _ => Err(format!("Unknown tool: {}", tool_name))
        }
    }
}
```

### 6. Tauri Command Layer (변경 불필요)

**파일:** `src-tauri/src/commands/mcp_commands.rs`

**현재 구현이 그대로 유지됩니다:**

```rust
#[tauri::command]
pub async fn call_builtin_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
    request_id: Option<String>,
) -> MCPResponse {
    let request_id = request_id.map(serde_json::Value::String);
    get_mcp_manager()
        .call_builtin_tool(&server_name, &tool_name, arguments, request_id)
        .await
}
```

**이유:**

- Command는 단순 중계 역할 유지
- `MCPServerManager::call_builtin_tool()`이 `BuiltinServerRegistry::call_tool()` 호출
- Registry가 래핑 담당하므로 command 변경 불필요
- 기존 API 시그니처 유지로 frontend 영향 없음

### 7. Workspace Tools 테스트 업데이트

      content: Some(vec![MCPContent {
        type_: "text".to_string(),
        text: Some(format!("Error: {}", err)),
        resource: None,
      }]),
      structured_content: None,
      is_error: Some(true),
    }),
    error: None,

}
}

````

### 5. Workspace Module 리팩토링 (주요 예시)

**파일:** `src-tauri/src/mcp/builtin/workspace/tools/file_tools.rs`

```rust
// Before
pub async fn read_file_handler(
  workspace_path: &Path,
  file_path: String,
  request_id: String,
) -> Result<MCPResponse, String> {
  let full_path = workspace_path.join(&file_path);

  if !full_path.exists() {
    return Ok(MCPResponse {
      jsonrpc: "2.0".to_string(),
      id: request_id,
      result: None,
      error: Some(MCPError {
        code: -32000,
        message: "File not found".to_string(),
        data: None,
      }),
    });
  }

  let content = std::fs::read_to_string(&full_path)
    .map_err(|e| format!("Failed to read: {}", e))?;

  Ok(MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: request_id,
    result: Some(json!({
      "type": "text",
      "text": content,
    })),
    error: None,
  })
}

// After
pub async fn read_file_handler(
  workspace_path: &Path,
  file_path: String,
) -> Result<MCPResult, String> {
  let full_path = workspace_path.join(&file_path);

  if !full_path.exists() {
    return Ok(MCPResult {
      content: Some(vec![MCPContent {
        type_: "text".to_string(),
        text: Some("File not found".to_string()),
        resource: None,
      }]),
      structured_content: Some(json!({
        "path": file_path,
        "reason": "File does not exist",
      })),
      is_error: Some(true),
    });
  }

  let content = std::fs::read_to_string(&full_path)
    .map_err(|e| format!("Failed to read: {}", e))?;

  Ok(MCPResult {
    content: Some(vec![MCPContent {
      type_: "text".to_string(),
      text: Some(content.clone()),
      resource: None,
    }]),
    structured_content: Some(json!({
      "path": file_path,
      "size": content.len(),
    })),
    is_error: Some(false),
  })
}
````

### 6. Content Store Module 리팩토링 (주요 예시)

**파일:** `src-tauri/src/mcp/builtin/content_store/handlers.rs`

```rust
// Before
pub async fn add_content_handler(
  db: &ContentStoreDb,
  content: String,
  metadata: serde_json::Value,
  request_id: String,
) -> Result<MCPResponse, String> {
  let content_id = db.add(content, metadata).await
    .map_err(|e| format!("Failed to add: {}", e))?;

  Ok(MCPResponse {
    jsonrpc: "2.0".to_string(),
    id: request_id,
    result: Some(json!({
      "id": content_id,
      "status": "created",
    })),
    error: None,
  })
}

// After
pub async fn add_content_handler(
  db: &ContentStoreDb,
  content: String,
  metadata: serde_json::Value,
) -> Result<MCPResult, String> {
  let content_id = db.add(content.clone(), metadata).await
    .map_err(|e| format!("Failed to add: {}", e))?;

  Ok(MCPResult {
    content: Some(vec![MCPContent {
      type_: "text".to_string(),
      text: Some(format!("Content added with ID: {}", content_id)),
      resource: None,
    }]),
    structured_content: Some(json!({
      "id": content_id,
      "status": "created",
      "size": content.len(),
    })),
    is_error: Some(false),
  })
}
```

## 재사용 가능한 연관 코드

### 1. Rust MCPResult Builder Helper

**파일:** `src-tauri/src/mcp/utils/mod.rs` (신규 또는 추가)

```rust
pub fn create_mcp_success(
  text: String,
  data: Option<serde_json::Value>,
) -> MCPResult {
  MCPResult {
    content: Some(vec![MCPContent {
      type_: "text".to_string(),
      text: Some(text),
      resource: None,
    }]),
    structured_content: data,
    is_error: Some(false),
  }
}

pub fn create_mcp_error(
  message: String,
  context: Option<serde_json::Value>,
) -> MCPResult {
  MCPResult {
    content: Some(vec![MCPContent {
      type_: "text".to_string(),
      text: Some(message),
      resource: None,
    }]),
    structured_content: context,
    is_error: Some(true),
  }
}
```

### 2. Rust Error Mapping

**파일:** `src-tauri/src/mcp/builtin/utils.rs`

```rust
impl From<std::io::Error> for String {
  fn from(err: std::io::Error) -> Self {
    format!("IO Error: {}", err)
  }
}

pub fn io_error_to_mcp_result(err: std::io::Error) -> MCPResult {
  create_mcp_error(
    format!("File operation failed: {}", err),
    Some(json!({
      "error_kind": format!("{:?}", err.kind()),
      "error_message": err.to_string(),
    }))
  )
}
```

### 3. Workspace Tools 테스트 유틸

**파일:** `src-tauri/src/mcp/builtin/workspace/tests/mod.rs`

```rust
pub async fn test_read_file_success() {
  // Create temp file
  let result = read_file_handler(&workspace, "test.txt".to_string()).await;

  assert!(result.is_ok());
  let mcp_result = result.unwrap();
  assert_eq!(mcp_result.is_error, Some(false));
  assert!(mcp_result.content.is_some());
  assert_eq!(mcp_result.content.unwrap().len(), 1);
}

pub async fn test_read_file_not_found() {
  let result = read_file_handler(&workspace, "nonexistent.txt".to_string()).await;

  assert!(result.is_ok());
  let mcp_result = result.unwrap();
  assert_eq!(mcp_result.is_error, Some(true));
}
```

## Test Code 추가 및 수정 필요 부분 가이드

### 1. Rust Unit Tests 업데이트

**파일:** `src-tauri/src/mcp/builtin/workspace/tests/mod.rs`

```rust
#[tokio::test]
async fn test_read_file_handler() {
  // Before: expected MCPResponse
  // After: expect MCPResult

  let result = read_file_handler(&workspace, "test.txt".to_string()).await;

  assert!(result.is_ok());
  let mcp_result = result.unwrap();

  // MCPResult 검증
  assert!(mcp_result.content.is_some());
  assert_eq!(mcp_result.is_error, Some(false));

  let content = mcp_result.content.unwrap();
  assert_eq!(content.len(), 1);
  assert_eq!(content[0].type_, "text");
}

#[tokio::test]
async fn test_read_file_not_found() {
  let result = read_file_handler(&workspace, "nonexistent.txt".to_string()).await;

  assert!(result.is_ok());
  let mcp_result = result.unwrap();

  assert_eq!(mcp_result.is_error, Some(true));
  assert!(mcp_result.structured_content.is_some());
}
```

### 2. Tauri Command Integration Test

**파일:** `src-tauri/src/commands/tests/mcp_commands.rs`

```rust
#[tokio::test]
async fn test_call_builtin_tool_wrapping() {
  // command에서 MCPResponse가 올바르게 래핑되는지 확인

  let response = call_builtin_tool(
    "workspace".to_string(),
    "read_file".to_string(),
    json!({ "path": "test.txt" }),
    "req-123".to_string(),
    app_handle,
  ).await;

  assert!(response.is_ok());
  let mcp_response = response.unwrap();

  // MCPResponse 검증
  assert_eq!(mcp_response.jsonrpc, "2.0");
  assert_eq!(mcp_response.id, "req-123");
  assert!(mcp_response.result.is_some());

  let mcp_result = mcp_response.result.unwrap();
  assert_eq!(mcp_result.is_error, Some(false));
  assert!(mcp_result.content.is_some());
}
```

### 3. Frontend Integration Test

**파일:** `src/lib/__tests__/builtin-tools.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { callBuiltinTool } from '@/lib/backend/builtin-tools';

describe('Builtin Tools Integration', () => {
  it('should receive MCPResponse from Rust builtin tools', async () => {
    const response = await callBuiltinTool('workspace', 'read_file', {
      path: 'test.txt',
    });

    // MCPResponse 구조 검증
    expect(response.jsonrpc).toBe('2.0');
    expect(response.id).toBeDefined();
    expect(response.result).toBeDefined();
    expect(response.result?.isError).toBe(false);
    expect(response.result?.content).toBeDefined();
  });

  it('should handle error results', async () => {
    const response = await callBuiltinTool('workspace', 'read_file', {
      path: 'nonexistent.txt',
    });

    expect(response.result?.isError).toBe(true);
    expect(response.result?.structuredContent).toBeDefined();
  });
});
```

### 테스트 실행 가이드

```bash
# Rust 테스트
cargo test -p tauri --lib mcp::builtin

# Frontend 테스트
pnpm test builtin-tools

# Integration 테스트
pnpm test --integration

# Watch mode
cargo watch -x test

# Coverage
cargo tarpaulin
```

## Clarification Q-list

### Q1. Error Handling 전략 ✅ ANSWERED

**질문:** Rust에서도 에러를 `MCPResult.isError`로 처리할지, `Err(String)` 반환 후 Registry에서 MCPError로 변환할지?

**✅ 채택 답변: Option B - `Result<MCPResult, String>` 사용**

**이유:**

- ✅ Rust idiom: `Result` 타입으로 실패 가능한 작업 표현
- ✅ `?` operator로 깔끔한 error propagation
- ✅ Business logic error와 protocol error 분리
- ✅ Registry가 error code mapping 담당 (-32603 등)
- ✅ Type safety: compiler가 error handling 강제

**구현:**

```rust
// Handler
pub async fn handle_read_file(&self, path: &str) -> Result<MCPResult, String> {
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    Ok(MCPResult {
        content: vec![MCPContent::text(content)],
        is_error: false,
    })
}

// Registry wrapping
match server.call_tool(tool_name, args).await {
    Ok(result) => MCPResponse { result: Some(result), error: None, ... },
    Err(e) => MCPResponse {
        result: None,
        error: Some(MCPError { code: -32603, message: e, ... }),
        ...
    }
}
```

### Q2. Handler 함수 호출 방식 ✅ ANSWERED

**질문:** Handler를 직접 호출할지, 동적 dispatch (match tool_name)를 계속 사용할지?

**✅ 채택 답변: Option A - Match-based dispatch 유지 (현재 방식)**

**이유:**

- ✅ 현재 codebase와 일관성 유지
- ✅ 단순하고 명확한 구조
- ✅ Compile-time type checking
- ✅ 추가 abstraction overhead 없음
- ✅ Handler registry는 over-engineering (현재 규모에서)

**구현:**

```rust
#[async_trait]
impl BuiltinMCPServer for WorkspaceServer {
    async fn call_tool(&self, tool_name: &str, args: Value)
        -> Result<MCPResult, String>
    {
        match tool_name {
            "poll_process" => self.handle_poll_process(args).await,
            "read_process_output" => self.handle_read_process_output(args).await,
            "list_processes" => self.handle_list_processes(args).await,
            _ => Err(format!("Unknown tool: {}", tool_name))
        }
    }
}
```

### Q3. Request ID 관리 ✅ ANSWERED

**질문:** Frontend에서 request_id를 생성할지, Backend에서 생성할지?

**✅ 채택 답변: Option A - Frontend에서 생성 (현재 유지)**

**이유:**

- ✅ 현재 구현과 일관성
- ✅ Frontend에서 request-response correlation 가능
- ✅ 디버깅 용이 (frontend 로그와 매칭)
- ✅ Web MCP Worker 패턴과 일치
- ✅ `BuiltinServerRegistry`가 fallback ID 생성 (None일 경우)

**구현:**

```typescript
// Frontend (builtin-tools.ts)
export async function callBuiltinTool(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
  requestId?: string,
): Promise<MCPResponse<unknown>> {
  const id = requestId ?? createId(); // Frontend 생성
  return safeInvoke<MCPResponse<unknown>>('call_builtin_tool', {
    serverName,
    toolName,
    arguments: args,
    requestId: id,
  });
}
```

```rust
// Registry (builtin/mod.rs)
pub async fn call_tool(..., request_id: Option<Value>) -> MCPResponse {
    let id = request_id.unwrap_or_else(||
        Value::String(uuid::Uuid::new_v4().to_string())
    );  // ← Fallback 생성
    // ...
}
```

### Q4. 기존 테스트 마이그레이션 ✅ ANSWERED

**질문:** 기존 Rust tests와 Frontend tests를 모두 업데이트할지, 점진적으로 진행할지?

**✅ 채택 답변: Option B - 점진적 Phase별 진행**

**Phase 1: Foundation (Week 1)**

- ✅ Trait 정의 변경
- ✅ Registry 래핑 로직 추가
- ✅ Helper 함수 구현
- ✅ Core handlers 5-10개 변경
- ✅ 변경된 handlers 테스트 업데이트

**Phase 2: Workspace Server (Week 2)**

- ✅ 나머지 Workspace handlers 변경
- ✅ WorkspaceServer integration tests
- ✅ E2E 테스트 (핵심 기능)

**Phase 3: Content Store + Others (Week 3)**

- ✅ ContentStoreServer handlers 변경
- ✅ 모든 unit tests 업데이트
- ✅ Frontend integration tests

**Phase 4: Cleanup (Week 4)**

- ✅ Deprecated code 제거
- ✅ Documentation 업데이트
- ✅ Performance validation
- ✅ `pnpm refactor:validate` 통과

**이유:**

- 단계별 검증으로 리스크 최소화
- 각 phase 완료 후 사용자 피드백 반영 가능
- 병렬 작업 가능 (다른 기능 개발과 동시 진행)
- Rollback이 용이함

### Q5. Breaking Change 관리 ✅ ANSWERED

**질문:** Rust types 변경을 어떻게 배포할지?

**✅ 채택 답변: Option B - Direct Replacement (Internal API)**

**이유:**

- ✅ Builtin tools는 **internal API** (외부 노출 안됨)
- ✅ Frontend API 변경 없음 (여전히 `MCPResponse` 수신)
- ✅ Tauri command 시그니처 유지
- ✅ Breaking change가 실제로는 없음 (내부 리팩토링)
- ✅ v2 API 유지 비용 불필요

**Migration Strategy:**

```markdown
1. Trait 변경 → 모든 impl 업데이트 (compile error로 누락 방지)
2. Phase별 handler 변경
3. Tests 업데이트
4. CHANGELOG에 internal refactoring 명시
```

**CHANGELOG 예시:**

```markdown
## [Internal] Refactoring

### Changed (Internal Only - No API Impact)

- Refactored Rust builtin tool handlers to return `MCPResult` instead of `MCPResponse`
- Centralized JSON-RPC wrapping in `BuiltinServerRegistry`
- Standardized error handling across all builtin tools

### Notes

- **No frontend API changes** - all tools continue to return `MCPResponse`
- **No breaking changes** - internal refactoring only
- Improved code maintainability and consistency
```

---

## Implementation Migration Plan

### Phase 1: Foundation (Week 1) - Priority: CRITICAL

**Goal:** Establish new architecture without breaking existing functionality

**Tasks:**

1. **Update `BuiltinMCPServer` trait** (builtin/mod.rs)
   - Change `call_tool` return type to `Result<MCPResult, String>`
   - Remove `request_id` parameter from trait method
   - Update trait documentation

2. **Add Registry wrapping logic** (builtin/mod.rs)
   - Implement wrapping in `BuiltinServerRegistry::call_tool()`
   - Handle success/error cases
   - Test with existing tools

3. **Create helper functions** (mcp/utils/mod.rs or builtin/utils.rs)
   - `create_mcp_success(text, data)` → `MCPResult`
   - `create_mcp_error(message, context)` → `MCPResult`
   - `MCPContent::text(content)` helper

4. **Refactor 3-5 simple handlers** (proof of concept)
   - Pick simple handlers (e.g., ping, list operations)
   - Update to return `Result<MCPResult>`
   - Remove `request_id` generation
   - Verify tests pass

5. **Update critical tests**
   - Registry wrapping tests
   - Updated handler tests
   - Integration smoke tests

**Success Criteria:**

- ✅ All code compiles
- ✅ Existing tests pass
- ✅ New pattern proven with 3-5 handlers
- ✅ No regression in functionality

### Phase 2: Workspace Server (Week 2) - Priority: HIGH

**Goal:** Complete workspace tool handlers refactoring

**Tasks:**

1. **Terminal/Process handlers** (10-12 functions)
   - `handle_poll_process`
   - `handle_read_process_output`
   - `handle_list_processes`
   - Related terminal management

2. **File operation handlers** (5-8 functions)
   - File read/write/delete
   - Directory operations
   - Path utilities

3. **Code execution handlers** (3-5 functions)
   - Shell execution
   - Python/Node execution
   - Environment management

4. **WorkspaceServer trait impl update**
   - Update `call_tool` match statement
   - Remove all `MCPResponse` returns
   - Update error handling

5. **Tests**
   - Unit tests for all changed handlers
   - Integration tests for tool execution flow
   - E2E tests for critical workflows

**Success Criteria:**

- ✅ All workspace handlers refactored
- ✅ All workspace tests pass
- ✅ E2E file operations working
- ✅ Terminal/process management working

### Phase 3: Content Store + Cleanup (Week 3) - Priority: MEDIUM

**Goal:** Complete remaining handlers and polish

**Tasks:**

1. **ContentStore handlers** (12-15 functions)
   - Search operations
   - CRUD operations
   - Metadata management

2. **ContentStoreServer trait impl**
   - Update `call_tool` implementation
   - Error handling standardization

3. **Remaining tests**
   - All content store tests
   - Frontend integration tests
   - Cross-feature tests

4. **Code cleanup**
   - Remove deprecated helper functions
   - Update comments/documentation
   - Remove unused imports

**Success Criteria:**

- ✅ All builtin handlers refactored
- ✅ 100% test coverage maintained
- ✅ No deprecated code remaining
- ✅ Code review approved

### Phase 4: Documentation & Validation (Week 4) - Priority: LOW

**Goal:** Polish and document changes

**Tasks:**

1. **Documentation updates**
   - Update API documentation
   - Add migration guide for future developers
   - Update architecture diagrams

2. **Performance validation**
   - Benchmark tool execution times
   - Memory usage comparison
   - Latency measurements

3. **Final validation**
   - `pnpm refactor:validate` passes
   - `cargo test --all` passes
   - Integration test suite passes
   - Manual E2E testing

4. **CHANGELOG**
   - Document internal refactoring
   - Note no breaking changes
   - List improvements

**Success Criteria:**

- ✅ All validation checks pass
- ✅ Documentation complete
- ✅ Performance benchmarks acceptable
- ✅ Ready for merge to main

---

## References

### Related Documentation

- `docs/history/refactoring_20251116_2114.md` - Web MCP Architecture Refactoring
- `refactoring_plan_submission_guide.md` - Plan writing guide
- `.github/copilot-instructions.md` - Project guidelines

### Key Files to Modify

**Rust (src-tauri/src/):**

- `mcp/types.rs` - MCPResult, MCPResponse types
- `mcp/builtin/mod.rs` - Main builtin module
- `mcp/builtin/workspace/mod.rs` - Workspace handlers (20+ functions)
- `mcp/builtin/workspace/tools/*.rs` - Tool-specific handlers
- `mcp/builtin/content_store/handlers.rs` - Content Store handlers (12+ functions)
- `commands/mcp_commands.rs` - Tauri command layer (wrapping logic)
- `mcp/utils/mod.rs` - Helper utilities (new MCPResult builders)

**TypeScript (src/):**

- `lib/backend/builtin-tools.ts` - Frontend caller (minor changes)
- `lib/mcp/protocol/response.ts` - Type definitions (review)
- `__tests__/builtin-tools.test.ts` - Tests (update expected types)

### Architecture Patterns

- Web MCP refactoring: Worker wraps MCPResult → MCPResponse
- Rust builtin: Command wraps MCPResult → MCPResponse
- **Unified Goal:** Frontend receives consistent MCPResponse structure

### Standards

- Rust: Follow project coding style (`rustfmt`, `clippy`)
- TypeScript: ESLint, Prettier compliant
- Tests: Both unit (Rust) and integration (Frontend) required
