# Rust MCPServerConfig Migration Strategy - 하위 호환성 유지

**작성일**: 2025-10-29  
**목적**: 현재 Rust의 Legacy MCPServerConfig를 포괄적인 형식으로 마이그레이션하면서 하위 호환성 완벽 보장

---

## 📋 목차

1. [현재 구조 분석](#1-현재-구조-분석)
2. [제안된 포괄적 구조](#2-제안된-포괄적-구조)
3. [하위 호환성 전략](#3-하위-호환성-전략)
4. [마이그레이션 단계](#4-마이그레이션-단계)
5. [테스트 전략](#5-테스트-전략)

---

## 1. 현재 구조 분석

### 1.1 현재 Rust MCPServerConfig

**파일**: `src-tauri/src/mcp/types.rs`

```rust
/// Represents the configuration for an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    /// The unique name of the server.
    pub name: String,

    /// The command to execute to start the server (for stdio transport).
    pub command: Option<String>,

    /// An array of arguments to pass to the command.
    pub args: Option<Vec<String>>,

    /// Environment variables to set for the server process.
    pub env: Option<HashMap<String, String>>,

    /// The transport protocol ("stdio", "http", "websocket"). Defaults to "stdio".
    #[serde(default = "default_transport")]
    pub transport: String,

    /// The URL of the server (for http or websocket transports).
    pub url: Option<String>,

    /// The port number of the server (for http or websocket transports).
    pub port: Option<u16>,
}

fn default_transport() -> String {
    "stdio".to_string()
}
```

### 1.2 현재 사용 패턴

#### Frontend에서 전달하는 형식

```typescript
// 현재 사용 중인 형식 (간단한 stdio 전용)
{
  "mcpServers": {
    "my-server": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem"],
      "env": { "ROOT_PATH": "/home/user" }
    }
  }
}
```

#### Rust Backend 처리

```rust
// src-tauri/src/commands/mcp_commands.rs:104
// Claude format 처리 (현재 코드)
for (name, server_config) in mcp_servers.iter() {
    let mut server_value = server_config.clone();
    if let serde_json::Value::Object(ref mut obj) = server_value {
        obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
        obj.insert("transport".to_string(), serde_json::Value::String("stdio".to_string()));
    }
    let server_cfg: MCPServerConfig = serde_json::from_value(server_value)?;
    server_list.push(server_cfg);
}
```

### 1.3 현재 제약사항

**지원되는 것**:

- ✓ Stdio transport (완전 지원)
- ✓ HTTP/WebSocket transport (설정만 가능, 실제 연결 미구현)
- ✓ 간단한 command/args/env 설정

**지원되지 않는 것**:

- ✗ OAuth 토큰 관리
- ✗ HTTP 헤더 커스터마이징
- ✗ Transport별 상세 설정
- ✗ 준비 상태(readiness) 확인
- ✗ 입력 변수(inputVars) 메타데이터
- ✗ MCP Capabilities 정보

---

## 2. 제안된 포괄적 구조

### 2.1 Enhanced MCPServerConfig (V2)

```rust
// src-tauri/src/mcp/types.rs - 새로운 구조

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Transport type discriminator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MCPTransportType {
    Stdio,
    Http,
    Sse,
}

/// Stdio transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StdioTransportConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

/// HTTP/SSE transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteTransportConfig {
    pub url: String,

    /// Optional authentication token (Bearer, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    #[serde(default = "default_auth_token_type")]
    pub auth_token_type: String, // "bearer", "basic", "custom"

    /// Custom HTTP headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,

    /// Token expiration info (Unix timestamp in ms)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_env: Option<HashMap<String, String>>,
}

fn default_auth_token_type() -> String {
    "bearer".to_string()
}

/// Unified transport configuration (discriminated union)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase")]
pub enum TransportConfig {
    Stdio(StdioTransportConfig),
    Http(RemoteTransportConfig),
    Sse(RemoteTransportConfig),
}

/// MCP Capabilities (from MCP spec)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptCapabilities>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Input variable metadata (for user-provided values)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputVar {
    pub name: String,
    pub description: String,
    pub required: bool,

    #[serde(rename = "type")]
    pub var_type: String, // "env", "cmd", "header"

    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg_index: Option<usize>, // For cmd type

    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_key: Option<String>, // For header type

    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>, // Documentation link
}

/// Enhanced MCP Server Configuration (V2)
/// Supports both legacy stdio-only format and new comprehensive format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfigV2 {
    pub name: String,

    /// Transport configuration (discriminated union)
    #[serde(flatten)]
    pub transport_config: TransportConfig,

    /// MCP protocol version
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,

    /// MCP capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<MCPCapabilities>,

    /// Input variables metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_vars: Option<Vec<InputVar>>,

    /// Server info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

fn default_protocol_version() -> String {
    "2025-06-18".to_string()
}

/// Legacy MCPServerConfig (현재 구조 유지 - 하위 호환성)
/// **DEPRECATED**: Use MCPServerConfigV2 instead
#[derive(Debug, Clone, Serialize, Deserialize)]
#[deprecated(since = "2.0.0", note = "Use MCPServerConfigV2 instead")]
pub struct MCPServerConfig {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    #[serde(default = "default_transport")]
    pub transport: String,
    pub url: Option<String>,
    pub port: Option<u16>,
}

fn default_transport() -> String {
    "stdio".to_string()
}

/// Conversion trait: Legacy -> V2 (자동 마이그레이션)
impl From<MCPServerConfig> for MCPServerConfigV2 {
    fn from(legacy: MCPServerConfig) -> Self {
        match legacy.transport.as_str() {
            "stdio" => {
                let command = legacy.command.unwrap_or_else(|| "npx".to_string());
                let args = legacy.args.unwrap_or_default();

                MCPServerConfigV2 {
                    name: legacy.name,
                    transport_config: TransportConfig::Stdio(StdioTransportConfig {
                        command,
                        args,
                        cwd: None,
                        env: legacy.env,
                    }),
                    protocol_version: default_protocol_version(),
                    capabilities: None,
                    input_vars: None,
                    server_info: None,
                }
            }
            "http" => {
                let url = legacy.url.unwrap_or_else(|| {
                    format!("http://localhost:{}", legacy.port.unwrap_or(8080))
                });

                MCPServerConfigV2 {
                    name: legacy.name,
                    transport_config: TransportConfig::Http(RemoteTransportConfig {
                        url,
                        auth_token: None,
                        auth_token_type: default_auth_token_type(),
                        headers: None,
                        expires_at: None,
                        custom_env: legacy.env,
                    }),
                    protocol_version: default_protocol_version(),
                    capabilities: None,
                    input_vars: None,
                    server_info: None,
                }
            }
            "sse" | "websocket" => {
                let url = legacy.url.unwrap_or_else(|| {
                    format!("ws://localhost:{}", legacy.port.unwrap_or(8080))
                });

                MCPServerConfigV2 {
                    name: legacy.name,
                    transport_config: TransportConfig::Sse(RemoteTransportConfig {
                        url,
                        auth_token: None,
                        auth_token_type: default_auth_token_type(),
                        headers: None,
                        expires_at: None,
                        custom_env: legacy.env,
                    }),
                    protocol_version: default_protocol_version(),
                    capabilities: None,
                    input_vars: None,
                    server_info: None,
                }
            }
            _ => {
                // Fallback to stdio for unknown transport
                MCPServerConfigV2 {
                    name: legacy.name,
                    transport_config: TransportConfig::Stdio(StdioTransportConfig {
                        command: "npx".to_string(),
                        args: vec![],
                        cwd: None,
                        env: legacy.env,
                    }),
                    protocol_version: default_protocol_version(),
                    capabilities: None,
                    input_vars: None,
                    server_info: None,
                }
            }
        }
    }
}
```

---

## 3. 하위 호환성 전략

### 3.1 핵심 원칙

**✅ 100% 하위 호환성 보장**:

1. 기존 `MCPServerConfig` 구조 유지 (deprecated 마킹만)
2. 자동 변환 로직 (`From<MCPServerConfig> for MCPServerConfigV2`)
3. Frontend는 기존 형식 그대로 전송 가능
4. 점진적 마이그레이션 지원

### 3.2 Deserialization 전략 (양방향 지원)

```rust
// src-tauri/src/mcp/types.rs

/// Unified config wrapper that accepts both legacy and V2 formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MCPServerConfigWrapper {
    V2(MCPServerConfigV2),
    Legacy(MCPServerConfig),
}

impl MCPServerConfigWrapper {
    /// Convert to unified V2 format
    pub fn into_v2(self) -> MCPServerConfigV2 {
        match self {
            Self::V2(v2) => v2,
            Self::Legacy(legacy) => legacy.into(),
        }
    }
}
```

### 3.3 Command Handler 업데이트

```rust
// src-tauri/src/commands/mcp_commands.rs - 수정

#[tauri::command]
pub async fn list_tools_from_config(
    config: serde_json::Value,
) -> Result<HashMap<String, Vec<MCPTool>>, String> {
    println!("🚀 [TAURI] list_tools_from_config called!");

    // Support for both V2 and legacy formats
    let servers_config = parse_server_configs(&config)?;

    let manager = get_mcp_manager();
    let mut tools_by_server: HashMap<String, Vec<MCPTool>> = HashMap::new();

    for server_cfg_v2 in servers_config {
        let server_name = server_cfg_v2.name.clone();

        if !manager.is_server_alive(&server_name).await {
            println!("🚀 [TAURI] Starting server: {server_name}");
            if let Err(e) = manager.start_server_v2(server_cfg_v2.clone()).await {
                eprintln!("❌ [TAURI] Failed to start server {server_name}: {e}");
                tools_by_server.insert(server_name, Vec::new());
                continue;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }

        match manager.list_tools(&server_name).await {
            Ok(tools) => {
                println!("✅ [TAURI] Found {} tools for server '{}'", tools.len(), server_name);
                tools_by_server.insert(server_name, tools);
            }
            Err(e) => {
                eprintln!("❌ [TAURI] Error listing tools for '{server_name}': {e}");
                tools_by_server.insert(server_name, Vec::new());
            }
        }
    }

    Ok(tools_by_server)
}

/// Parse server configs from JSON (supports both legacy and V2 formats)
fn parse_server_configs(
    config: &serde_json::Value,
) -> Result<Vec<MCPServerConfigV2>, String> {
    let mut server_list = Vec::new();

    // Try Claude format (mcpServers object)
    if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
        println!("🚀 [TAURI] Processing mcpServers format");

        for (name, server_config) in mcp_servers.iter() {
            let mut server_value = server_config.clone();

            // Add name field if missing
            if let serde_json::Value::Object(ref mut obj) = server_value {
                if !obj.contains_key("name") {
                    obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                }

                // Auto-add transport field for legacy format
                if !obj.contains_key("transport") && !obj.contains_key("transport_config") {
                    obj.insert("transport".to_string(), serde_json::Value::String("stdio".to_string()));
                }
            }

            // Try V2 format first, fallback to legacy
            let config_wrapper: MCPServerConfigWrapper = serde_json::from_value(server_value)
                .map_err(|e| format!("Invalid server config for '{name}': {e}"))?;

            server_list.push(config_wrapper.into_v2());
        }
    }
    // Try servers array format
    else if let Some(servers_array) = config.get("servers").and_then(|v| v.as_array()) {
        println!("🚀 [TAURI] Processing servers array format");

        for server_value in servers_array {
            let config_wrapper: MCPServerConfigWrapper = serde_json::from_value(server_value.clone())
                .map_err(|e| format!("Invalid server config: {e}"))?;

            server_list.push(config_wrapper.into_v2());
        }
    }
    else {
        return Err("Invalid config: missing mcpServers object or servers array".to_string());
    }

    println!("🚀 [TAURI] Parsed {} servers", server_list.len());
    Ok(server_list)
}
```

### 3.4 MCPServerManager 확장

```rust
// src-tauri/src/mcp/server.rs - 추가

impl MCPServerManager {
    /// Start server with V2 configuration (supports all transports)
    pub async fn start_server_v2(&self, config: MCPServerConfigV2) -> Result<String> {
        match config.transport_config {
            TransportConfig::Stdio(stdio_config) => {
                self.start_stdio_server_v2(config.name, stdio_config).await
            }
            TransportConfig::Http(remote_config) => {
                self.start_http_server(config.name, remote_config).await
            }
            TransportConfig::Sse(remote_config) => {
                self.start_sse_server(config.name, remote_config).await
            }
        }
    }

    /// Start stdio server with V2 config
    async fn start_stdio_server_v2(
        &self,
        name: String,
        config: StdioTransportConfig,
    ) -> Result<String> {
        // Create command with rmcp
        let cmd = Command::new(&config.command).configure(|cmd| {
            for arg in &config.args {
                cmd.arg(arg);
            }

            if let Some(cwd) = &config.cwd {
                cmd.current_dir(cwd);
            }

            if let Some(env) = &config.env {
                for (key, value) in env {
                    cmd.env(key, value);
                }
            }
        });

        let transport = TokioChildProcess::new(cmd)?;
        let client = ().serve(transport).await?;
        info!("Successfully connected to MCP server: {}", name);

        let connection = MCPConnection { client };
        self.connections.lock().await.insert(name.clone(), connection);

        Ok(format!("Server '{}' started successfully", name))
    }

    /// Start HTTP MCP server (NEW)
    async fn start_http_server(
        &self,
        name: String,
        config: RemoteTransportConfig,
    ) -> Result<String> {
        // TODO: Implement HTTP transport using reqwest
        info!("HTTP server configured: {} at {}", name, config.url);

        // For now, just store the config - actual implementation needed
        Ok(format!("HTTP server '{}' configured at {}", name, config.url))
    }

    /// Start SSE MCP server (NEW)
    async fn start_sse_server(
        &self,
        name: String,
        config: RemoteTransportConfig,
    ) -> Result<String> {
        // TODO: Implement SSE transport
        info!("SSE server configured: {} at {}", name, config.url);

        Ok(format!("SSE server '{}' configured at {}", name, config.url))
    }

    /// Legacy start_server for backward compatibility
    pub async fn start_server(&self, legacy_config: MCPServerConfig) -> Result<String> {
        let v2_config: MCPServerConfigV2 = legacy_config.into();
        self.start_server_v2(v2_config).await
    }
}
```

---

## 4. 마이그레이션 단계

### Phase 1: 타입 시스템 준비 (1-2일)

**작업**:

- [ ] `MCPServerConfigV2` 타입 정의
- [ ] `TransportConfig` enum 정의
- [ ] `MCPServerConfigWrapper` 정의
- [ ] `From<MCPServerConfig>` trait 구현
- [ ] Unit tests 작성

**하위 호환성 보장**:

- ✅ 기존 `MCPServerConfig` 그대로 유지
- ✅ Legacy format 자동 변환

### Phase 2: Command Handler 업데이트 (2-3일)

**작업**:

- [ ] `parse_server_configs()` 함수 구현
- [ ] `list_tools_from_config` 업데이트
- [ ] Deserialization 테스트

**하위 호환성 보장**:

- ✅ 기존 Frontend 코드 변경 없이 작동
- ✅ Legacy format 자동 파싱

### Phase 3: MCPServerManager 확장 (3-5일)

**작업**:

- [ ] `start_server_v2()` 메서드 추가
- [ ] `start_stdio_server_v2()` 구현
- [ ] `start_http_server()` 구현 (기본)
- [ ] `start_sse_server()` 구현 (기본)
- [ ] Legacy `start_server()` 래퍼 유지

**하위 호환성 보장**:

- ✅ 기존 `start_server()` API 유지
- ✅ 내부적으로 V2로 변환

### Phase 4: HTTP/SSE Transport 구현 (5-7일)

**작업**:

- [ ] `reqwest` HTTP client 통합
- [ ] Bearer token authentication
- [ ] Custom headers 지원
- [ ] SSE event stream 처리
- [ ] Connection pooling

**새로운 기능**:

- ✨ Remote MCP 완전 지원
- ✨ OAuth 토큰 인증

### Phase 5: Frontend 점진적 마이그레이션 (선택사항)

**작업**:

- [ ] TypeScript 타입 업데이트
- [ ] V2 format 사용 권장
- [ ] Documentation 업데이트

**하위 호환성 보장**:

- ✅ Legacy format 계속 지원
- ✅ 점진적 마이그레이션 가능

---

## 5. 테스트 전략

### 5.1 Unit Tests

```rust
// src-tauri/src/mcp/types.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_to_v2_stdio() {
        let legacy = MCPServerConfig {
            name: "test-server".to_string(),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "package".to_string()]),
            env: Some([("KEY".to_string(), "value".to_string())].iter().cloned().collect()),
            transport: "stdio".to_string(),
            url: None,
            port: None,
        };

        let v2: MCPServerConfigV2 = legacy.into();

        assert_eq!(v2.name, "test-server");
        match v2.transport_config {
            TransportConfig::Stdio(stdio) => {
                assert_eq!(stdio.command, "npx");
                assert_eq!(stdio.args.len(), 2);
                assert!(stdio.env.is_some());
            }
            _ => panic!("Expected Stdio transport"),
        }
    }

    #[test]
    fn test_deserialize_legacy_format() {
        let json = r#"{
            "name": "test-server",
            "command": "npx",
            "args": ["-y", "package"],
            "transport": "stdio"
        }"#;

        let wrapper: MCPServerConfigWrapper = serde_json::from_str(json).unwrap();
        let v2 = wrapper.into_v2();

        assert_eq!(v2.name, "test-server");
    }

    #[test]
    fn test_deserialize_v2_format() {
        let json = r#"{
            "name": "test-server",
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "package"],
            "protocol_version": "2025-06-18"
        }"#;

        let v2: MCPServerConfigV2 = serde_json::from_str(json).unwrap();

        assert_eq!(v2.name, "test-server");
        assert_eq!(v2.protocol_version, "2025-06-18");
    }

    #[test]
    fn test_deserialize_v2_http_format() {
        let json = r#"{
            "name": "github-mcp",
            "transport": "http",
            "url": "https://api.github.com/mcp",
            "auth_token": "ghp_token123",
            "headers": {
                "Authorization": "Bearer ghp_token123"
            }
        }"#;

        let v2: MCPServerConfigV2 = serde_json::from_str(json).unwrap();

        assert_eq!(v2.name, "github-mcp");
        match v2.transport_config {
            TransportConfig::Http(http) => {
                assert_eq!(http.url, "https://api.github.com/mcp");
                assert!(http.auth_token.is_some());
            }
            _ => panic!("Expected HTTP transport"),
        }
    }
}
```

### 5.2 Integration Tests

```rust
// src-tauri/src/commands/mcp_commands.rs - tests

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parse_legacy_claude_format() {
        let config = serde_json::json!({
            "mcpServers": {
                "my-server": {
                    "command": "npx",
                    "args": ["-y", "package"]
                }
            }
        });

        let servers = parse_server_configs(&config).unwrap();

        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "my-server");
    }

    #[tokio::test]
    async fn test_parse_v2_format() {
        let config = serde_json::json!({
            "mcpServers": {
                "github-mcp": {
                    "transport": "http",
                    "url": "https://api.github.com",
                    "auth_token": "token123"
                }
            }
        });

        let servers = parse_server_configs(&config).unwrap();

        assert_eq!(servers.len(), 1);
        match &servers[0].transport_config {
            TransportConfig::Http(http) => {
                assert_eq!(http.url, "https://api.github.com");
            }
            _ => panic!("Expected HTTP transport"),
        }
    }

    #[tokio::test]
    async fn test_mixed_format() {
        // Legacy stdio + V2 http 동시 지원
        let config = serde_json::json!({
            "mcpServers": {
                "local-fs": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem"]
                },
                "github": {
                    "transport": "http",
                    "url": "https://api.github.com",
                    "auth_token": "ghp_123"
                }
            }
        });

        let servers = parse_server_configs(&config).unwrap();

        assert_eq!(servers.len(), 2);

        // First should be stdio (legacy converted)
        match &servers[0].transport_config {
            TransportConfig::Stdio(_) => {}
            _ => panic!("Expected Stdio for first server"),
        }

        // Second should be HTTP (native V2)
        match &servers[1].transport_config {
            TransportConfig::Http(_) => {}
            _ => panic!("Expected HTTP for second server"),
        }
    }
}
```

### 5.3 E2E Tests

**시나리오**:

1. ✅ Legacy stdio format으로 서버 시작 → 정상 작동
2. ✅ V2 stdio format으로 서버 시작 → 정상 작동
3. ✅ V2 HTTP format으로 서버 연결 → 정상 작동
4. ✅ Mixed format (legacy + V2) → 모두 정상 작동
5. ✅ 기존 Frontend 코드 변경 없이 작동 확인

---

## 6. 하위 호환성 보장 체크리스트

### ✅ API 레벨

- [x] 기존 `start_server(MCPServerConfig)` 메서드 유지
- [x] 기존 `list_tools_from_config()` 시그니처 유지
- [x] Frontend에서 전송하는 JSON 형식 그대로 지원

### ✅ 데이터 레벨

- [x] Legacy `MCPServerConfig` 타입 유지 (deprecated만 마킹)
- [x] `mcpServers` 객체 형식 계속 지원
- [x] `servers` 배열 형식 계속 지원
- [x] stdio transport 기본값 유지

### ✅ 동작 레벨

- [x] Legacy format → V2 자동 변환
- [x] 기존 stdio 서버 시작 로직 유지
- [x] 에러 처리 로직 동일하게 유지

### ✅ 마이그레이션 레벨

- [x] 점진적 마이그레이션 가능
- [x] Frontend 코드 변경 없이 Backend만 업데이트 가능
- [x] 혼합 형식(Legacy + V2) 동시 지원

---

## 7. 롤아웃 전략

### Option A: Shadow Testing (추천)

```rust
// 기존 로직과 신규 로직 병행 실행, 결과 비교
pub async fn list_tools_from_config(config: Value) -> Result<...> {
    // 기존 로직
    let legacy_result = parse_server_configs_legacy(&config);

    // 신규 로직
    let v2_result = parse_server_configs(&config);

    // 결과 비교 로그
    if let (Ok(legacy), Ok(v2)) = (&legacy_result, &v2_result) {
        if legacy.len() != v2.len() {
            warn!("⚠️ Migration discrepancy: legacy={}, v2={}", legacy.len(), v2.len());
        }
    }

    // V2 사용
    v2_result
}
```

### Option B: Feature Flag

```rust
// Cargo.toml
[features]
mcp-v2 = []

// Code
#[cfg(feature = "mcp-v2")]
use crate::mcp::types::MCPServerConfigV2;

#[cfg(not(feature = "mcp-v2"))]
use crate::mcp::types::MCPServerConfig;
```

### Option C: Gradual Rollout (가장 안전)

1. **Week 1**: V2 타입 추가, 자동 변환 로직 구현
2. **Week 2**: Unit/Integration tests, shadow testing
3. **Week 3**: Beta 배포, 모니터링
4. **Week 4**: 전체 배포
5. **Month 2**: Legacy format deprecation 경고
6. **Month 6**: Legacy format 제거 (선택)

---

## 8. 요약

### ✅ 하위 호환성 100% 보장

**Yes, 완벽한 하위 호환성 유지 가능합니다!**

**핵심 전략**:

1. **타입 병존**: Legacy `MCPServerConfig`와 V2 `MCPServerConfigV2` 동시 지원
2. **자동 변환**: `From` trait으로 Legacy → V2 자동 변환
3. **Wrapper 패턴**: `MCPServerConfigWrapper`로 양방향 deserialization
4. **API 유지**: 기존 command handler 시그니처 그대로 유지
5. **점진적 마이그레이션**: Frontend 코드 변경 없이 Backend만 업데이트 가능

**장점**:

- ✅ 기존 코드 변경 없음
- ✅ 점진적 마이그레이션
- ✅ Remote MCP 지원 추가
- ✅ 더 나은 타입 안전성
- ✅ 확장 가능한 구조

**단점**:

- 초기 구현 복잡도 증가 (1-2주)
- 코드베이스에 두 가지 형식 공존 (일시적)

**권장사항**:

- Phase 1-3 먼저 구현 (stdio 완벽 호환)
- Shadow testing으로 안정성 검증
- Phase 4에서 HTTP/SSE 추가 (신규 기능)
- Frontend는 필요시 천천히 마이그레이션

---

## 참고 자료

- [MCP Config Comparison Analysis](./MCP_CONFIG_COMPARISON_ANALYSIS.md)
- [API Response Schema](./API_RESPONSE_SCHEMA_FOR_USER_ACTIVATED_MCPs.md)
- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [Serde Untagged Enums](https://serde.rs/enum-representations.html#untagged)
