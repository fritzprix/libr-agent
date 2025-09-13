# 🌐 Web MCP (Model Context Protocol) System

A Web Worker-based implementation of the Model Context Protocol that enables MCP servers to run directly in the browser without Node.js or Python dependencies.

## Overview

The Web MCP system provides:

- **Dependency-free execution**: Run MCP servers using only browser Web Workers
- **Standard MCP compliance**: Full compatibility with MCP tool calling and discovery
- **Native integration**: Seamless integration with Tauri APIs for system operations
- **Type safety**: Complete TypeScript support with strong typing
- **Performance**: Efficient module caching and message processing

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
│   React App     │    │   MCP Proxy      │    │   Web Worker        │
│                 │    │                  │    │                     │
│ - Components    │◄──►│ - Message Router │◄──►│ - MCP Server        │
│ - Hooks         │    │ - Error Handling │    │ - Tool Execution    │
│ - Context       │    │ - Timeout Mgmt   │    │ - Module Loading    │
└─────────────────┘    └──────────────────┘    └─────────────────────┘
                                  │
                                  ▼
                       ┌──────────────────┐
                       │  MCP Modules     │
                       │                  │
                       │ - calculator.ts  │
                       │ - filesystem.ts  │
                       │ - custom.ts      │
                       └──────────────────┘
```

## Core Components

### 1. MCP Worker (`mcp-worker.ts`)

The main Web Worker that:

- Dynamically loads MCP server modules
- Handles message passing with the main thread
- Manages server lifecycle and caching
- Provides error handling and logging

### 2. MCP Proxy (`mcp-proxy.ts`)

Communication interface that:

- Abstracts Web Worker messaging
- Provides Promise-based API
- Handles timeouts and retries
- Manages worker lifecycle

### 3. MCP Modules (`modules/`)

Individual MCP servers implementing the `WebMCPServer` interface:

- `calculator.ts` - Arithmetic operations
- `filesystem.ts` - File system operations using Tauri APIs
- Custom modules can be easily added

## Usage

### Basic Setup

```typescript
import { WebMCPProvider } from '@/context/WebMCPContext';

function App() {
  return (
    <WebMCPProvider
      workerPath="/workers/mcp-worker.js"
      servers={['calculator', 'filesystem']}
      autoLoad={true}
    >
      <YourComponents />
    </WebMCPProvider>
  );
}
```

### Using Web MCP Tools

```typescript
import { useWebMCPTools } from '@/hooks/use-web-mcp';

function CalculatorComponent() {
  const { executeCall, availableTools } = useWebMCPTools();

  const calculate = async () => {
    const result = await executeCall('calculator', 'add', { a: 5, b: 3 });
    console.log(result); // { result: 8, operation: 'addition', ... }
  };

  return (
    <div>
      <button onClick={calculate}>Calculate 5 + 3</button>
      <p>Available tools: {availableTools.length}</p>
    </div>
  );
}
```

### Creating Custom MCP Servers

```typescript
// modules/my-server.ts
import {
  WebMCPServer,
  MCPTool,
  createObjectSchema,
  createStringSchema,
} from '../../mcp-types';

const tools: MCPTool[] = [
  {
    name: 'greet',
    description: 'Greet a person',
    inputSchema: createObjectSchema({
      properties: {
        name: createStringSchema({ description: 'Person name' }),
      },
      required: ['name'],
    }),
  },
];

async function callTool(name: string, args: unknown): Promise<unknown> {
  const params = args as { name: string };

  switch (name) {
    case 'greet':
      return { message: `Hello, ${params.name}!` };
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

const myServer: WebMCPServer = {
  name: 'my-server',
  description: 'A custom greeting server',
  version: '1.0.0',
  tools,
  callTool,
};

export default myServer;
```

## Integration with Unified MCP

The Web MCP system integrates seamlessly with the existing Tauri MCP system through the Unified MCP Provider:

```typescript
import { UnifiedMCPProvider } from '@/context/UnifiedMCPContext';

// Automatically handles both Tauri MCP and Web MCP tools
function App() {
  return (
    <MCPServerProvider>           {/* Tauri MCP */}
      <WebMCPProvider>            {/* Web Worker MCP */}
        <UnifiedMCPProvider>      {/* Unified interface */}
          <YourApp />
        </UnifiedMCPProvider>
      </WebMCPProvider>
    </MCPServerProvider>
  );
}
```

## Available Hooks

### `useWebMCP()`

Core Web MCP functionality access.

### `useWebMCPTools()`

Simplified tool operations and status checking.

### `useWebMCPManagement()`

Server loading and system management.

### `useUnifiedMCP()`

Combined access to both Tauri and Web MCP systems.

## Tool Naming Convention

Web MCP tools follow the pattern: `serverName__toolName`

Examples:

- `calculator__add`
- `calculator__multiply`
- `filesystem__read_file`
- `filesystem__write_file`

This ensures tool names are unique across different MCP systems.

## MCP Response Contract (Important)

When a tool is called via the dynamic server proxy (see `WebMCPContext.tsx`),
the return value is derived from the MCP response using this precedence:

1. If `result.structuredContent` exists, return it as-is (preferred for typed data)
2. Else if `result.content[0].type === 'text'`, try `JSON.parse(text)`, fall back to raw string
3. Else return the raw `result` object

This contract enables two common styles of server implementations to work seamlessly:

- Structured responses (e.g., content-store): servers use `createMCPStructuredResponse(text, structuredContent)`
  and callers receive `structuredContent` directly (e.g., `{ storeId: 'store_...' }`).
- Text-only responses (e.g., planning-server): servers use `createMCPTextResponse(text)` and callers receive
  either parsed JSON (if the text is valid JSON) or the plain string.

### Example: content-store

```ts
// Server side: returns structuredContent with typed fields
return createMCPStructuredResponse(
  `Content added with ID: ${result.contentId}`,
  result as Record<string, unknown>,
);

// Client side: using dynamic proxy
const { server } = useWebMCPServer<ContentStoreServer>('content-store');
const created = await server.createStore({ metadata: { sessionId } });
// created => { storeId: string, createdAt: string | Date }

const added = await server.addContent({
  storeId,
  fileUrl,
  metadata: { filename },
});
// added => { storeId, contentId, filename, mimeType, size, lineCount, preview, uploadedAt, chunkCount }
```

### Example: planning-server

```ts
// Server side: returns text only
return createMCPTextResponse(`Todo added: "${name}"`);

// Client side: using dynamic proxy
const { server } = useWebMCPServer('planning');
const textOrJson = await server.add_todo({ name: 'Write docs' });
// textOrJson => either parsed JSON if valid, or plain string
```

### Why structuredContent first?

Structured content provides a stable contract for typed data, avoiding brittle text parsing
and enabling safer client code. Servers that can return structured data should prefer it.

### Accessing Both Text and Data

When you need access to both the human-readable text and structured data from a response,
use the `ToolResult` helper utility:

```ts
import { toToolResult, ToolResult } from '@/lib/web-mcp/tool-result';

// Get both text and structured data
const response = await proxy.callTool('content-store', 'addContent', args);
const both = toToolResult<AddContentOutput>(response);

// Access structured data (if available)
if (both.data) {
  console.log('Store ID:', both.data.storeId);
  console.log('Content ID:', both.data.contentId);
}

// Access human-readable text (if available)
if (both.text) {
  console.log('Summary:', both.text);
}

// Access raw MCP response envelope for custom processing
console.log('Raw response:', both.raw);
```

This is particularly useful for:

- Displaying user-friendly messages while processing structured data
- Debugging responses by comparing text and data formats
- Building UIs that need both presentation and data layers

## Error Handling

The system provides comprehensive error handling:

```typescript
try {
  const result = await executeCall('calculator', 'divide', { a: 10, b: 0 });
} catch (error) {
  console.error('Tool execution failed:', error.message);
  // Handle error appropriately
}
```

Errors are automatically converted to standard MCP Response format with proper error codes and messages.

## Performance Considerations

- **Module Caching**: Loaded MCP servers are cached in the worker
- **Message Batching**: Multiple tool calls can be processed efficiently
- **Timeout Management**: Configurable timeouts prevent hanging operations
- **Memory Management**: Proper cleanup of resources and event listeners

## Security

- **Sandboxed Execution**: Tools run in isolated Web Worker environment
- **Tauri Integration**: File system operations use Tauri's secure APIs
- **Input Validation**: All tool arguments are validated against JSON schemas
- **Error Isolation**: Errors in one tool don't affect others

## Development

### Adding New MCP Servers

1. Create a new file in `modules/` directory
2. Implement the `WebMCPServer` interface
3. Define tools with proper JSON schemas
4. Export the server as default export
5. Add server name to the provider configuration

### Testing

Use the Web MCP Demo component (`/tools/webmcp-demo`) to test:

- Server loading and initialization
- Tool discovery and execution
- Error handling scenarios
- Integration with the unified system

### Debugging

Enable debug logging:

```typescript
import { getLogger } from '@/lib/logger';
const logger = getLogger('WebMCP');
logger.setLevel('debug');
```

## Examples

### Calculator Operations

```typescript
// Basic arithmetic
await executeCall('calculator', 'add', { a: 5, b: 3 });
await executeCall('calculator', 'multiply', { a: 4, b: 7 });

// Advanced operations
await executeCall('calculator', 'power', { base: 2, exponent: 8 });
await executeCall('calculator', 'sqrt', { value: 16 });
await executeCall('calculator', 'factorial', { n: 5 });
```

### File System Operations

```typescript
// File operations
await executeCall('filesystem', 'write_file', {
  path: '/tmp/hello.txt',
  content: 'Hello World!',
});

await executeCall('filesystem', 'read_file', {
  path: '/tmp/hello.txt',
});

// Directory operations
await executeCall('filesystem', 'list_directory', {
  path: '/home/user',
  recursive: false,
});
```

## Future Enhancements

- **HTTP Transport**: Support for HTTP-based MCP servers
- **WebSocket Support**: Real-time bidirectional communication
- **Module Hot Reloading**: Dynamic module updates without restart
- **Performance Monitoring**: Built-in performance metrics
- **Advanced Caching**: Intelligent caching strategies

## Troubleshooting

### Common Issues

1. **Worker not loading**: Check worker path and Vite configuration
2. **Module not found**: Ensure module is in `modules/` directory
3. **Tool execution timeout**: Increase timeout in proxy configuration
4. **Permission errors**: Verify Tauri permissions for file operations

### Debug Steps

1. Check browser console for errors
2. Verify Web MCP provider initialization
3. Test individual tool calls in demo component
4. Check worker status in DevTools
5. Review Tauri logs for file system operations

## Contributing

When contributing to the Web MCP system:

1. Follow TypeScript best practices
2. Add comprehensive error handling
3. Include proper JSON schema validation
4. Write tests for new functionality
5. Update documentation and examples
