# Built-in MCP Servers

SynapticFlow now includes built-in MCP servers that provide essential functionality without requiring external installations. These servers are implemented in Rust and offer excellent performance and security.

## Available Built-in Servers

### 1. Filesystem Server (`builtin.filesystem`)

Provides secure file system operations within the current working directory.

#### Available Tools

- `read_file` - Read the contents of a file
- `write_file` - Write content to a file
- `list_directory` - List contents of a directory

#### Security Features

- Path validation to prevent directory traversal attacks
- Access restricted to current working directory and subdirectories
- File size limits to prevent memory exhaustion
- Automatic parent directory creation for write operations

### 2. Sandbox Server (`builtin.sandbox`)

Executes code in isolated sandbox environments.

#### Available Tools

- `execute_python` - Execute Python code safely
- `execute_typescript` - Execute TypeScript code using ts-node

#### Security Features

- Isolated temporary directory execution
- Environment variable isolation
- Execution timeout limits (1-60 seconds, default 30)
- Code size limits (max 10KB)
- Process cleanup and resource management

## Usage Examples

### Frontend TypeScript

```typescript
import { tauriMCPClient } from '@/lib/tauri-mcp-client';

// List all available built-in servers
const servers = await tauriMCPClient.listBuiltinServers();
console.log('Built-in servers:', servers);
// Output: ['builtin.filesystem', 'builtin.sandbox']

// List all built-in tools
const tools = await tauriMCPClient.listBuiltinTools();
console.log(
  'Available tools:',
  tools.map((t) => t.name),
);

// Read a file
const readResult = await tauriMCPClient.callBuiltinTool(
  'builtin.filesystem',
  'read_file',
  { path: 'README.md' },
);

// Write a file
const writeResult = await tauriMCPClient.callBuiltinTool(
  'builtin.filesystem',
  'write_file',
  {
    path: 'output.txt',
    content: 'Hello from SynapticFlow!',
  },
);

// List directory contents
const listResult = await tauriMCPClient.callBuiltinTool(
  'builtin.filesystem',
  'list_directory',
  { path: '.' },
);

// Execute Python code
const pythonResult = await tauriMCPClient.callBuiltinTool(
  'builtin.sandbox',
  'execute_python',
  {
    code: 'print("Hello from Python!")\nprint(2 + 2)',
    timeout: 10,
  },
);

// Execute TypeScript code
const tsResult = await tauriMCPClient.callBuiltinTool(
  'builtin.sandbox',
  'execute_typescript',
  {
    code: 'console.log("Hello from TypeScript!"); console.log(2 + 2);',
    timeout: 5,
  },
);

// Use unified API (works with both external and built-in servers)
const unifiedTools = await tauriMCPClient.listAllToolsUnified();
const unifiedResult = await tauriMCPClient.callToolUnified(
  'builtin.filesystem',
  'read_file',
  { path: 'package.json' },
);
```

### Direct Tauri Commands

```typescript
import { invoke } from '@tauri-apps/api/core';

// List built-in servers
const servers = (await invoke('list_builtin.servers')) as string[];

// List built-in tools
const tools = (await invoke('list_builtin.tools')) as MCPTool[];

// Call built-in tool
const result = (await invoke('call_builtin.tool', {
  serverName: 'builtin.filesystem',
  toolName: 'read_file',
  args: { path: 'example.txt' },
})) as MCPResponse;

// Unified API - automatically routes to appropriate server
const unifiedResult = (await invoke('call_tool_unified', {
  serverName: 'builtin.sandbox',
  toolName: 'execute_python',
  args: { code: 'print("Hello!")', timeout: 10 },
})) as MCPResponse;
```

## Response Format

All tool calls return responses in the standard MCP format:

```typescript
interface MCPResponse {
  jsonrpc: string; // Always "2.0"
  id?: any; // Request identifier
  result?: {
    content: Array<{
      type: 'text';
      text: string;
    }>;
    isError?: boolean; // For sandbox execution results
  };
  error?: {
    code: number;
    message: string;
    data?: any;
  };
}
```

### Successful Response Example

```json
{
  "jsonrpc": "2.0",
  "id": "uuid-string",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "File contents here..."
      }
    ]
  }
}
```

### Error Response Example

```json
{
  "jsonrpc": "2.0",
  "id": "uuid-string",
  "error": {
    "code": -32603,
    "message": "Security error: Path '../../../etc/passwd' attempts to access outside working directory"
  }
}
```

## Security Considerations

### Filesystem Server Security

1. **Path Validation**: All paths are validated and cleaned to prevent directory traversal
2. **Scope Restriction**: Access is limited to the current working directory and its subdirectories
3. **File Size Limits**: Maximum file size is 10MB to prevent memory exhaustion
4. **Parent Directory Creation**: Automatic and safe creation of parent directories for write operations

### Sandbox Server Security

1. **Process Isolation**: Code runs in separate processes with isolated environments
2. **Timeout Protection**: Execution is limited by configurable timeouts (max 60 seconds)
3. **Resource Limits**: Code size is limited to 10KB to prevent abuse
4. **Environment Isolation**: Environment variables are cleared except for essential PATH variables
5. **Temporary Workspace**: Each execution gets a clean temporary directory that's automatically cleaned up

## Error Codes

| Code   | Description                                                    |
| ------ | -------------------------------------------------------------- |
| -32601 | Tool not found                                                 |
| -32602 | Invalid parameters (missing required parameters)               |
| -32603 | Internal error (security violations, execution failures, etc.) |

## Integration with AI Services

Built-in tools work seamlessly with AI services and can be included in tool calling workflows:

```typescript
// AI services can discover and use built-in tools
const allTools = await tauriMCPClient.listAllToolsUnified();

// Tools are automatically formatted for AI service consumption
const aiCompatibleTools = allTools.map((tool) => ({
  name: tool.name,
  description: tool.description,
  parameters: tool.input_schema,
}));
```

## Future Enhancements

The built-in server architecture is designed for easy extensibility. Planned additions include:

1. **HTTP Client Server**: REST API calling capabilities
2. **Database Server**: SQLite operations
3. **Image Processing Server**: Basic image manipulation
4. **Text Processing Server**: Regular expressions and text transformations
5. **System Info Server**: System information queries

## Performance Benefits

Built-in servers offer several advantages over external MCP servers:

1. **No Installation Required**: Available immediately without setup
2. **Native Performance**: Rust implementation provides excellent speed
3. **Lower Latency**: No IPC overhead for communication
4. **Better Resource Management**: Integrated lifecycle management
5. **Enhanced Security**: Compiled security controls and validation
