# MCP Web Worker Refactoring Plan

## Background

Current MCP (Model Context Protocol) implementations are primarily built with Node.js or Python, which leads to several issues:

- **Dependency Complexity**: Requires Node.js/Python installation on end-user machines
- **Deployment Constraints**: Difficult to integrate MCP servers directly into web applications
- **Security Limitations**: Restricted integration of AI services with various features on client-side

Especially in special cases like Tauri, built-in MCP support is possible, but the current structure makes deployment and management complex.

## Purpose

Implement an MCP-compatible layer using Web Workers to achieve:

1. **Dependency Elimination**: Run MCP servers with browser runtime only, without Node.js/Python
2. **Built-in Support**: Provide embedded MCP servers within applications
3. **Easy Deployment**: Deploy directly alongside web applications
4. **Standard Compliance**: Adhere to MCP tool calling and discovery standards

## Requirements

### Platform Support
- **Tauri**: Built-in MCP server support in desktop applications
- **Next.js**: Web Worker-based MCP support in web applications
- **Compatibility**: Utilize Web Worker APIs that work in browser environments

### Functional Requirements
- **Tool Discovery**: Query available tool lists
- **Tool Calling**: Execute specific tools and return results
- **Dynamic Loading**: Load MCP server modules at runtime
- **Type Safety**: Strong typing support based on TypeScript

### Non-Functional Requirements
- **Simple Interface**: MCP-specialized simple API rather than generic worker
- **Performance**: Module caching and efficient message processing
- **Extensibility**: Easy addition of new MCP server modules

## Current State Analysis

### Existing Worker System
- ✅ Dynamic module loading (`import('./modules/${moduleName}')`)
- ✅ Message-based RPC (`WorkerProxy`, `self.onmessage`)
- ✅ Callback support and type safety
- ✅ Module caching and lifecycle management

### Areas for Improvement
- 🔄 Generic API → Specialize to MCP-dedicated API
- 🔄 Complex callback system → Simplify to MCP standard interface
- 🔄 Arbitrary function calls → Standard `listTools`, `callTool` methods

## Refactoring Plan

### 1. MCP Type Definitions
```typescript
// types/mcp.ts
export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: object; // JSON Schema
}

export interface MCPServer {
  name: string;
  description?: string;
  version?: string;
  tools: ToolDefinition[];
  callTool: (name: string, args: any) => Promise<any>;
}

export interface MCPMessage {
  id: string;
  type: 'listTools' | 'callTool' | 'ping';
  serverName: string;
  args?: any;
}

export interface MCPResponse {
  id: string;
  result?: any;
  error?: string;
}
```

### 2. MCP Module Standard Interface
```typescript
// modules/calculator.ts
import { MCPServer } from '@/types/mcp';

const tools = [
  {
    name: "add",
    description: "Add two numbers",
    inputSchema: {
      type: "object",
      properties: {
        a: { type: "number" },
        b: { type: "number" }
      },
      required: ["a", "b"]
    }
  }
];

async function callTool(name: string, args: any): Promise<any> {
  switch(name) {
    case "add":
      return { result: args.a + args.b };
    default:
      throw new Error(`Unknown tool: ${name}`);
  }
}

const server: MCPServer = {
  name: "calculator",
  description: "Basic calculator operations",
  version: "1.0.0",
  tools,
  callTool
};

export default server;
```

### 3. Worker Message Processing (MCP-Specific)
```typescript
// worker.ts
import { MCPServer, MCPMessage, MCPResponse } from '@/types/mcp';

const mcpServers = new Map<string, MCPServer>();

async function loadMCPServer(serverName: string): Promise<MCPServer> {
  if (mcpServers.has(serverName)) {
    return mcpServers.get(serverName)!;
  }

  const mod = await import(`./modules/${serverName}`);
  const server = mod.default as MCPServer;
  mcpServers.set(serverName, server);
  return server;
}

async function handleMCPMessage(message: MCPMessage): Promise<MCPResponse> {
  const { id, type, serverName, args } = message;

  try {
    const server = await loadMCPServer(serverName);
    
    let result;
    switch(type) {
      case 'listTools':
        result = server.tools;
        break;
      case 'callTool':
        result = await server.callTool(args.name, args.arguments);
        break;
      default:
        throw new Error(`Unknown MCP method: ${type}`);
    }
    
    return { id, result };
  } catch (error) {
    return { id, error: error.message };
  }
}

self.onmessage = async (event: MessageEvent<MCPMessage>) => {
  const response = await handleMCPMessage(event.data);
  self.postMessage(response);
};
```

### 4. MCP Provider Usage
```typescript
// Usage in React
<WebMCPProvider servers={["calculator", "file-system"]}>
   <ExampleSubscriber />
</WebMCPProvider>

function ExampleSubscriber() {
    const { availableTools, executeCall } = useWebMCP();
    
    // Query all available tools
    const tools = await availableTools();
    
    // Execute specific tool
    const result = await executeCall("calculator", "add", { a: 1, b: 2 });
}
```

### 5. Implementation Phases

#### Phase 1: Core Infrastructure
1. MCP type definitions (`types/mcp.ts`)
2. MCP Worker implementation (`mcp-worker.ts`)
3. MCP Proxy implementation (`mcp-proxy.ts`)

#### Phase 2: Provider & Hooks
1. MCP Context Provider (`useMCPProvider`)
2. MCP Hooks (`useMCP`, `useWebMCP`)
3. Basic test module implementation

#### Phase 3: Actual MCP Servers
1. Calculator server
2. File System server (utilizing Tauri API)
3. Additional utility servers

### 6. Differences from Existing System

| Existing Worker System      | MCP-Specific System                  |
| --------------------------- | ------------------------------------ |
| Arbitrary function calls    | Standardized `listTools`, `callTool` |
| Complex callback system     | Simple Promise-based                 |
| Generic module interface    | MCP server standard interface        |
| Per-function proxy creation | Per-server proxy creation            |

## Expected Results

### User Experience
- MCP functionality available without dependency installation
- Built-in tools within web applications
- Compatibility with other MCP clients through standard MCP interface

### Developer Experience
- Simple MCP server module implementation
- Type-safe tool invocation
- Easy deployment and management

### Extensibility
- Easy addition of new MCP server modules
- System tools utilizing Tauri native APIs
- Versatility that works in web environments