# External MCP Server Integration

## Overview

SynapticFlow provides comprehensive support for integrating external MCP (Model Context Protocol) servers, enabling unlimited tool expansion beyond built-in capabilities. This document details the architecture, implementation, and integration patterns for connecting external MCP servers.

## Architecture Overview

### Dual MCP Backend System

SynapticFlow implements a dual MCP backend architecture:

1. **External MCP Servers**: Third-party MCP servers running as child processes
2. **Built-in MCP Servers**: Native Rust implementations (content_store, workspace)

### Integration Flow

```
Frontend (React) → MCPServerContext → useRustBackend → rust-backend-client.ts → Tauri invoke → Rust Backend → MCPServerManager
```

## Frontend Integration

### MCPServerContext

The `MCPServerContext` provides React hooks for MCP server management:

```typescript
interface MCPServerContextType {
  availableTools: MCPTool[];
  getAvailableTools: () => MCPTool[];
  isLoading: boolean;
  error?: string;
  status: Record<string, boolean>;
  connectServers: (mcpConfigs: MCPConfig) => Promise<void>;
  executeToolCall: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  sampleFromModel: (
    serverName: string,
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
}
```

#### Tool Name Resolution

External MCP tools are accessed using a namespaced format:

```typescript
const executeToolCall = useCallback(async (toolCall: ToolCall) => {
  const aiProvidedToolName = toolCall.function.name; // e.g., "myServer__listFiles"

  const delimiter = '__';
  const parts = aiProvidedToolName.split(delimiter);
  const alias = parts[0]; // "myServer"
  const toolName = parts.slice(1).join(delimiter); // "listFiles"

  const serverName = aliasToIdTableRef.current.get(alias);
  const rawResponse = await callMCPTool(serverName, toolName, toolArguments);
  return rawResponse;
}, []);
```

### useMCPServer Hook

Simple React hook for accessing MCP server functionality:

```typescript
export const useMCPServer = () => {
  const context = useContext(MCPServerContext);
  if (context === undefined) {
    throw new Error('useMCPServer must be used within a MCPServerProvider');
  }
  return context;
};
```

## Backend Client Layer

### rust-backend-client.ts

Centralized client for all Tauri backend communication:

```typescript
export async function callTool(
  serverName: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResponse<unknown>> {
  return safeInvoke<MCPResponse<unknown>>('call_mcp_tool', {
    serverName,
    toolName,
    arguments: args,
  });
}

export async function listToolsFromConfig(config: {
  mcpServers?: Record<
    string,
    { command: string; args?: string[]; env?: Record<string, string> }
  >;
}): Promise<Record<string, MCPTool[]>> {
  return safeInvoke<Record<string, MCPTool[]>>('list_tools_from_config', {
    config,
  });
}
```

## Tauri Command Layer

### lib.rs - MCP Commands

Tauri commands that bridge frontend and Rust backend:

```rust
#[tauri::command]
async fn start_mcp_server(config: MCPServerConfig) -> Result<String, String> {
    get_mcp_manager()
        .start_server(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn call_mcp_tool(
    server_name: String,
    tool_name: String,
    arguments: serde_json::Value,
) -> MCPResponse {
    get_mcp_manager()
        .call_tool(&server_name, &tool_name, arguments)
        .await
}

#[tauri::command]
async fn list_tools_from_config(
    config: serde_json::Value,
) -> Result<std::collections::HashMap<String, Vec<mcp::MCPTool>>, String> {
    // Support for Claude format: handle mcpServers object
    let servers_config =
        if let Some(mcp_servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
            // Convert mcpServers object to MCPServerConfig array
            let mut server_list = Vec::new();
            for (name, server_config) in mcp_servers.iter() {
                let mut server_value = server_config.clone();
                if let serde_json::Value::Object(ref mut obj) = server_value {
                    obj.insert("name".to_string(), serde_json::Value::String(name.clone()));
                    obj.insert(
                        "transport".to_string(),
                        serde_json::Value::String("stdio".to_string()),
                    );
                }
                let server_cfg: mcp::MCPServerConfig = serde_json::from_value(server_value)
                    .map_err(|e| format!("Invalid server config: {e}"))?;
                server_list.push(server_cfg);
            }
            server_list
        } else {
            // Handle legacy servers array format
            // ...
        };

    // Start servers and collect tools
    let mut all_tools = std::collections::HashMap::new();
    for server_config in servers_config {
        match get_mcp_manager().start_server(server_config.clone()).await {
            Ok(_) => {
                match get_mcp_manager().list_tools(&server_config.name).await {
                    Ok(tools) => {
                        all_tools.insert(server_config.name, tools);
                    }
                    Err(e) => {
                        error!("Failed to list tools for server {}: {}", server_config.name, e);
                    }
                }
            }
            Err(e) => {
                error!("Failed to start server {}: {}", server_config.name, e);
            }
        }
    }

    Ok(all_tools)
}
```

## MCP Manager Layer

### MCPServerManager

Core component managing both external and built-in MCP servers:

```rust
#[derive(Debug)]
pub struct MCPServerManager {
    /// A map of active connections to external MCP servers, keyed by server name.
    connections: Arc<Mutex<HashMap<String, MCPConnection>>>,
    /// A registry for the built-in MCP servers.
    builtin_servers: Arc<Mutex<Option<crate::mcp::builtin::BuiltinServerRegistry>>>,
}
```

### External Server Lifecycle

#### Starting External Servers

```rust
async fn start_stdio_server(&self, config: MCPServerConfig) -> Result<String> {
    let command = config
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Command is required for stdio transport"))?;

    let default_args = vec![];
    let args = config.args.as_ref().unwrap_or(&default_args);

    // Create command with rmcp - configure returns the modified command
    let cmd = Command::new(command).configure(|cmd| {
        for arg in args {
            cmd.arg(arg);
        }

        // Set environment variables if any
        if let Some(env) = &config.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }
    });

    // Create transport and connect using RMCP pattern
    let transport = TokioChildProcess::new(cmd)?;
    debug!("Created transport for command: {command} {args:?}");

    let client = ().serve(transport).await?;
    info!("Successfully connected to MCP server: {}", config.name);

    let connection = MCPConnection { client };

    // Store connection
    {
        let mut connections = self.connections.lock().await;
        connections.insert(config.name.clone(), connection);
        debug!("Stored connection for server: {}", config.name);
    }

    Ok(format!(
        "Started and connected to MCP server: {}",
        config.name
    ))
}
```

#### Calling External Tools

```rust
pub async fn call_tool(
    &self,
    server_name: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> MCPResponse {
    let connections = self.connections.lock().await;

    // Generate a unique ID for this request
    let request_id = serde_json::Value::String(Uuid::new_v4().to_string());

    if let Some(connection) = connections.get(server_name) {
        // Use the rmcp API - CallToolRequestParam struct
        let args_map = if let serde_json::Value::Object(obj) = arguments {
            obj
        } else {
            serde_json::Map::new()
        };

        let call_param = CallToolRequestParam {
            name: tool_name.to_string().into(),
            arguments: Some(args_map),
        };

        match connection.client.call_tool(call_param).await {
            Ok(result) => {
                // Convert RMCP result to MCPResponse format
                let result_value = match serde_json::to_value(&result) {
                    Ok(value) => value,
                    Err(e) => {
                        return MCPResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(request_id),
                            result: None,
                            error: Some(MCPError {
                                code: -32603,
                                message: format!("Failed to serialize result: {e}"),
                                data: None,
                            }),
                        };
                    }
                };

                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: Some(result_value),
                    error: None,
                }
            }
            Err(e) => {
                error!("Tool call failed: {e}");
                MCPResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(request_id),
                    result: None,
                    error: Some(MCPError {
                        code: -32603,
                        message: format!("Tool execution failed: {e}"),
                        data: None,
                    }),
                }
            }
        }
    } else {
        // Fallback to built-in servers
        self.call_builtin_tool(server_name, tool_name, arguments).await
    }
}
```

## Server Configuration

### MCPServerConfig Structure

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MCPServerConfig {
    pub name: String,
    pub transport: String, // "stdio", "http", "websocket"
    pub command: Option<String>, // For stdio transport
    pub args: Option<Vec<String>>, // Command arguments
    pub env: Option<HashMap<String, String>>, // Environment variables
}
```

### Configuration Formats

#### Claude Format (mcpServers object)

```json
{
  "mcpServers": {
    "myServer": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    }
  }
}
```

#### Legacy Format (servers array)

```json
{
  "servers": [
    {
      "name": "myServer",
      "transport": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
    }
  ]
}
```

## Built-in MCP Servers

### BuiltinMCPServer Trait

```rust
#[async_trait]
pub trait BuiltinMCPServer: Send + Sync + std::fmt::Debug {
    /// Returns the unique name of the server (e.g., "workspace").
    fn name(&self) -> &str;

    /// Returns a brief description of the server's purpose.
    fn description(&self) -> &str;

    /// Returns the version of the server.
    fn version(&self) -> &str {
        "1.0.0"
    }

    /// Returns a list of all tools provided by this server.
    fn tools(&self) -> Vec<MCPTool>;

    /// Calls a tool on this server with the given arguments.
    async fn call_tool(&self, tool_name: &str, args: Value) -> MCPResponse;

    /// Returns a markdown-formatted string describing the server's current status and context.
    fn get_service_context(&self, _options: Option<&Value>) -> String {
        format!(
            "# {} Server Status\n\
            **Server**: {}\n\
            **Status**: Active\n\
            **Tools Available**: {}",
            self.name(),
            self.name(),
            self.tools().len()
        )
    }
}
```

### Available Built-in Servers

#### ContentStoreServer

- **Purpose**: Advanced file content management with semantic search
- **Features**: BM25 indexing, semantic search, session-based storage
- **Tools**: createStore, addContent, search, readContent, etc.

#### WorkspaceServer

- **Purpose**: Integrated workspace management and code execution
- **Features**: File operations, code execution, session isolation
- **Tools**: read_file, write_file, execute_python, execute_shell, etc.

## Communication Protocol

### MCP over stdio

External MCP servers communicate using the Model Context Protocol over standard input/output:

1. **Initialization**: Server receives `initialize` request
2. **Tool Discovery**: Client requests `tools/list` to discover available tools
3. **Tool Execution**: Client sends `tools/call` requests with parameters
4. **Response Handling**: Server responds with tool results or errors

### RMCP Library Integration

SynapticFlow uses the RMCP (Rust MCP) library for protocol handling:

```rust
use rmcp::{
    model::CallToolRequestParam,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};

// Create child process transport
let transport = TokioChildProcess::new(cmd)?;

// Create MCP client
let client = ().serve(transport).await?;

// Call tool
let call_param = CallToolRequestParam {
    name: tool_name.to_string().into(),
    arguments: Some(args_map),
};
let result = client.call_tool(call_param).await?;
```

## Error Handling

### Standardized Error Responses

All MCP operations return standardized `MCPResponse` objects:

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<MCPError>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
```

### Error Codes

- `-32601`: Method not found
- `-32602`: Invalid params
- `-32603`: Internal error
- `-32000`: Server error

## Security Considerations

### Process Isolation

- External MCP servers run as separate child processes
- No direct memory sharing with main application
- Environment variable sanitization

### Input Validation

- All tool arguments are validated before execution
- Command injection prevention through proper argument handling
- Path traversal protection in file operations

### Resource Limits

- Configurable timeouts for tool execution
- Memory usage monitoring for child processes
- Connection pooling and cleanup

## Performance Optimization

### Connection Management

- Persistent connections to external servers
- Connection pooling for frequently used servers
- Automatic reconnection on failure

### Caching

- Tool metadata caching to reduce discovery overhead
- Result caching for idempotent operations
- Session-based state management

### Async Processing

- Non-blocking tool execution using Tokio
- Concurrent server management
- Streaming responses for long-running operations

## Usage Examples

### Adding an External MCP Server

```typescript
// In your MCP configuration
const mcpConfig = {
  mcpServers: {
    filesystem: {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', '/allowed/path'],
    },
    git: {
      command: 'npx',
      args: [
        '-y',
        '@modelcontextprotocol/server-git',
        '--repository',
        '/path/to/repo',
      ],
    },
  },
};

// Connect servers
await connectServers(mcpConfig);
```

### Calling External Tools

```typescript
// Tool name format: serverName__toolName
const result = await executeToolCall({
  function: {
    name: 'filesystem__read_file',
    arguments: {
      path: '/path/to/file.txt',
    },
  },
});
```

## Troubleshooting

### Common Issues

#### Server Won't Start

- Check command path and permissions
- Verify environment variables
- Review server logs

#### Tool Calls Fail

- Validate tool arguments against schema
- Check server connectivity
- Review error messages in logs

#### Performance Issues

- Monitor resource usage
- Check for connection leaks
- Optimize tool argument sizes

### Debugging Tools

#### Server Logs

```bash
# Enable debug logging
RUST_LOG=debug cargo run
```

#### Connection Status

```typescript
const status = await checkServerStatus('serverName');
console.log('Server status:', status);
```

#### Tool Discovery

```typescript
const tools = await listMCPTools('serverName');
console.log('Available tools:', tools);
```

## Future Enhancements

### Planned Features

- HTTP/WebSocket transport support
- Server auto-discovery
- Tool hot-reloading
- Advanced authentication
- Server health monitoring

### Community Integration

- MCP server marketplace
- Shared server configurations
- Community-contributed tools
- Server performance benchmarking</content>
  <parameter name="filePath">/home/fritzprix/my_works/tauri-agent/docs/external-mcp-integration.md
