# MCP Built-in Server Module 개발 가이드

SynapticFlow의 Rust MCP 서버는 확장성을 위해 모듈화되어 있습니다. 이 가이드는 새로운 MCP 서버 모듈을 추가하는 방법을 구체적인 예시와 함께 설명합니다.

## 📁 현재 모듈 구조

```text
src-tauri/src/mcp/builtin/
├── mod.rs           # 서버 트레이트 정의 및 레지스트리
├── filesystem.rs    # 파일시스템 MCP 서버
├── sandbox.rs       # 코드 실행 MCP 서버
├── utils.rs         # 공통 유틸리티 (보안, 상수 등)
└── README.md        # 이 가이드
```

## 🏗️ MCP 서버 모듈 아키텍처

### 핵심 트레이트: `BuiltinMCPServer`

모든 MCP 서버는 다음 트레이트를 구현해야 합니다:

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync {
    fn name(&self) -> &str;                          // 서버 이름 (예: "builtin.example")
    fn description(&self) -> &str;                   // 서버 설명
    fn version(&self) -> &str { "1.0.0" }           // 버전 (기본값 제공)
    fn tools(&self) -> Vec<MCPTool>;                 // 제공하는 도구 목록
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse; // 도구 실행
}
```

### 도구 네이밍 규칙

- **서버 내부**: 도구명만 사용 (예: `"echo"`)
- **레지스트리**: 자동으로 `서버명__도구명` 형태로 변환 (예: `"builtin.example__echo"`)
- **프론트엔드**: `"builtin.example__echo"` 형태로 호출

## 🚀 새 MCP 서버 모듈 추가 단계별 가이드

### 1단계: 새 서버 파일 생성

예시: `example.rs` 파일을 생성합니다.

```rust
// src-tauri/src/mcp/builtin/example.rs

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{error, info};

use super::{
    utils::{constants::MAX_FILE_SIZE, SecurityValidator},
    BuiltinMCPServer,
};
use crate::mcp::{JSONSchema, JSONSchemaType, MCPError, MCPResponse, MCPTool};

/// 예시 MCP 서버 - 문자열 처리 도구들을 제공
pub struct ExampleServer {
    security: SecurityValidator,
}

impl ExampleServer {
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::default(),
        }
    }

    /// Echo 도구 정의 - 입력된 텍스트를 그대로 반환
    fn create_echo_tool() -> MCPTool {
        MCPTool {
            name: "echo".to_string(),
            title: Some("Echo Text".to_string()),
            description: "Echo the input text back to the user".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "text".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Text to echo".to_string()),
                                default: None,
                                examples: Some(vec![json!("Hello, world!")]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["text".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// 대문자 변환 도구 정의
    fn create_uppercase_tool() -> MCPTool {
        MCPTool {
            name: "uppercase".to_string(),
            title: Some("Convert to Uppercase".to_string()),
            description: "Convert input text to uppercase".to_string(),
            input_schema: JSONSchema {
                schema_type: JSONSchemaType::Object {
                    properties: Some({
                        let mut props = HashMap::new();
                        props.insert(
                            "text".to_string(),
                            JSONSchema {
                                schema_type: JSONSchemaType::String {
                                    min_length: Some(1),
                                    max_length: Some(1000),
                                    pattern: None,
                                    format: None,
                                },
                                title: None,
                                description: Some("Text to convert to uppercase".to_string()),
                                default: None,
                                examples: Some(vec![json!("hello world")]),
                                enum_values: None,
                                const_value: None,
                            },
                        );
                        props
                    }),
                    required: Some(vec!["text".to_string()]),
                    additional_properties: Some(false),
                    min_properties: None,
                    max_properties: None,
                },
                title: None,
                description: None,
                default: None,
                examples: None,
                enum_values: None,
                const_value: None,
            },
            output_schema: None,
            annotations: None,
        }
    }

    /// Echo 도구 실행 함수
    async fn handle_echo(&self, args: Value) -> MCPResponse {
        let request_id = Some(Value::String(uuid::Uuid::new_v4().to_string()));

        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(text) => text,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter 'text'".to_string(),
                        data: Some(json!({"parameter": "text"})),
                    }),
                };
            }
        };

        info!("Echo tool called with text: {}", text);

        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id,
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": text
                }]
            })),
            error: None,
        }
    }

    /// 대문자 변환 도구 실행 함수
    async fn handle_uppercase(&self, args: Value) -> MCPResponse {
        let request_id = Some(Value::String(uuid::Uuid::new_v4().to_string()));

        let text = match args.get("text").and_then(|v| v.as_str()) {
            Some(text) => text,
            None => {
                return MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request_id,
                    result: None,
                    error: Some(MCPError {
                        code: -32602,
                        message: "Missing required parameter 'text'".to_string(),
                        data: Some(json!({"parameter": "text"})),
                    }),
                };
            }
        };

        let uppercase_text = text.to_uppercase();
        info!("Uppercase tool called, converted '{}' to '{}'", text, uppercase_text);

        MCPResponse {
            jsonrpc: "2.0".to_string(),
            id: request_id,
            result: Some(json!({
                "content": [{
                    "type": "text",
                    "text": uppercase_text
                }]
            })),
            error: None,
        }
    }
}

#[async_trait]
impl BuiltinMCPServer for ExampleServer {
    fn name(&self) -> &str {
        "builtin.example"
    }

    fn description(&self) -> &str {
        "Example MCP server providing text processing tools"
    }

    fn tools(&self) -> Vec<MCPTool> {
        vec![
            Self::create_echo_tool(),
            Self::create_uppercase_tool(),
        ]
    }

    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse {
        match tool_name {
            "echo" => self.handle_echo(args).await,
            "uppercase" => self.handle_uppercase(args).await,
            _ => MCPResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(Value::String(uuid::Uuid::new_v4().to_string())),
                result: None,
                error: Some(MCPError {
                    code: -32601,
                    message: format!("Tool '{}' not found", tool_name),
                    data: Some(json!({"available_tools": ["echo", "uppercase"]})),
                }),
            },
        }
    }
}

impl Default for ExampleServer {
    fn default() -> Self {
        Self::new()
    }
}
```

### 2단계: `mod.rs`에 모듈 등록

```rust
// src-tauri/src/mcp/builtin/mod.rs

use crate::mcp::{MCPResponse, MCPTool};
use async_trait::async_trait;
use serde_json::Value;

pub mod filesystem;
pub mod sandbox;
pub mod utils;
pub mod example; // 새 모듈 추가

// ... 트레이트 정의 ...

impl BuiltinServerRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            servers: std::collections::HashMap::new(),
        };

        // Register built-in servers
        registry.register_server(Box::new(filesystem::FilesystemServer::new()));
        registry.register_server(Box::new(sandbox::SandboxServer::new()));
        registry.register_server(Box::new(example::ExampleServer::new())); // 새 서버 등록

        registry
    }
    
    // ... 나머지 코드 ...
}
```

### 3단계: 빌드 및 테스트

```bash
# Rust 코드 포맷팅
cd src-tauri
cargo fmt

# 린팅 확인
cargo clippy

# 빌드 테스트
cargo build
```

## 🔧 프론트엔드 연동

### 자동 도구 감지

프론트엔드는 자동으로 새 도구들을 감지합니다:

- `builtin.example__echo`
- `builtin.example__uppercase`

### 도구 호출 예시

```typescript
// 프론트엔드에서 도구 호출
const toolCall = {
  id: "req-123",
  type: "function",
  function: {
    name: "builtin.example__echo",
    arguments: JSON.stringify({ text: "Hello, SynapticFlow!" })
  }
};

const response = await executeToolCall(toolCall);
```

## 📋 개발 체크리스트

### ✅ 필수 구현 사항

- [ ] `BuiltinMCPServer` 트레이트 구현
- [ ] 고유한 서버 이름 설정 (`builtin.` prefix 사용)
- [ ] 각 도구별 `MCPTool` 정의 (스키마 포함)
- [ ] 도구 실행 함수 구현
- [ ] 에러 처리 및 로깅
- [ ] `mod.rs`에 서버 등록

### ✅ 권장 사항

- [ ] 입력 검증 및 보안 처리
- [ ] 명확한 에러 메시지
- [ ] 상세한 도구 설명 및 예시
- [ ] 단위 테스트 작성
- [ ] 문서화

## 🛡️ 보안 고려사항

### SecurityValidator 사용

파일 시스템 접근이 필요한 경우 `SecurityValidator`를 사용하세요:

```rust
use super::utils::SecurityValidator;

impl YourServer {
    pub fn new() -> Self {
        Self {
            security: SecurityValidator::default(),
        }
    }
    
    async fn handle_file_operation(&self, path: &str) -> MCPResponse {
        match self.security.validate_path(path) {
            Ok(safe_path) => {
                // 안전한 경로로 파일 작업 수행
            }
            Err(e) => {
                // 보안 에러 처리
            }
        }
    }
}
```

### 입력 크기 제한

```rust
use super::utils::constants::MAX_FILE_SIZE;

// 문자열 길이 제한
schema_type: JSONSchemaType::String {
    max_length: Some(MAX_FILE_SIZE as u32),
    // ...
}
```

## 🔍 디버깅 팁

### 로깅 활용

```rust
use tracing::{debug, info, warn, error};

async fn handle_tool(&self, args: Value) -> MCPResponse {
    info!("Tool called with args: {:?}", args);
    debug!("Processing tool logic...");
    
    match result {
        Ok(data) => {
            info!("Tool execution successful");
            // 성공 응답
        }
        Err(e) => {
            error!("Tool execution failed: {}", e);
            // 에러 응답
        }
    }
}
```

### 일반적인 문제 해결

1. **도구가 프론트엔드에 나타나지 않는 경우**
   - `mod.rs`에 모듈이 등록되었는지 확인
   - 서버 이름이 고유한지 확인
   - 빌드 에러가 없는지 확인

2. **도구 호출이 실패하는 경우**
   - 입력 스키마와 실제 파라미터가 일치하는지 확인
   - 에러 로그 확인
   - JSON 직렬화/역직렬화 문제 확인

## 📚 추가 참고 자료

- **기존 구현 참고**: `filesystem.rs`, `sandbox.rs`
- **유틸리티 함수**: `utils.rs`
- **MCP 타입 정의**: `src-tauri/src/mcp/mod.rs`
- **프론트엔드 연동**: `src/context/BuiltInToolContext.tsx`

## 🤝 기여 가이드

새로운 MCP 서버 모듈을 개발했다면:

1. 코드 품질 확인 (`cargo fmt`, `cargo clippy`)
2. 테스트 작성 및 실행
3. 문서화 업데이트
4. Pull Request 제출

---

**참고**: 이 가이드는 SynapticFlow 프로젝트의 현재 아키텍처를 기반으로 작성되었습니다. 프로젝트 구조 변경 시 가이드도 함께 업데이트해야 합니다.
