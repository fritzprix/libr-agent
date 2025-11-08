# MCP Manager Server

Built-in Web MCP server for managing MCP server configurations dynamically at runtime.

## Purpose

Allows AI agents to:

- List and search registered MCP servers
- Create new server configurations
- Connect servers to assistants or enable globally
- Disconnect servers from assistants

## Tools

### list_servers

List all registered MCP servers with pagination and filtering.

**Parameters:**

- `page` (number, optional): Page number (default: 1)
- `pageSize` (number, optional): Items per page (default: 20, -1 for all)
- `filterByAssistant` (boolean, optional): Filter by current assistant's servers
- `includeInactive` (boolean, optional): Include inactive servers (default: true)

**Output:**

- Paginated list of MCPServerEntity with metadata

**Example:**

```typescript
await proxy.callTool('mcp_manager', 'list_servers', {
  filterByAssistant: true,
  pageSize: 10,
});
```

### search_server

Search servers by name, description, or tags with relevance sorting.

**Parameters:**

- `query` (string, required): Search query
- `page`, `pageSize`: Same as list_servers
- `byNameOnly` (boolean, optional): Search only names (default: true)
- `includeInactive` (boolean, optional): Include inactive servers

**Output:**

- Paginated search results with query and relevance-sorted items

**Example:**

```typescript
await proxy.callTool('mcp_manager', 'search_server', {
  query: 'github',
  byNameOnly: false, // Search in descriptions and tags too
});
```

### create_server

Create a new MCP server configuration.

**Parameters:**

- `name` (string, required): Unique server name
- `description` (string, optional): Server description
- `transport` (object, required): Transport config
  - `type`: "stdio" or "http"
  - For stdio: `command`, `args?`, `env?`
  - For http: `url`, `headers?`
- `tags` (string[], optional): Tags for categorization

**Output:**

- Created MCPServerEntity with confirmation message

**Example (stdio):**

```typescript
await proxy.callTool('mcp_manager', 'create_server', {
  name: 'my-github-server',
  description: 'GitHub API integration',
  transport: {
    type: 'stdio',
    command: 'node',
    args: ['server.js'],
    env: { API_KEY: 'xxx' },
  },
  tags: ['github', 'api'],
});
```

**Example (http):**

```typescript
await proxy.callTool('mcp_manager', 'create_server', {
  name: 'weather-api',
  description: 'Weather data service',
  transport: {
    type: 'http',
    url: 'https://api.weather.com/mcp',
    headers: { 'X-API-Key': 'xxx' },
  },
  tags: ['weather', 'api'],
});
```

### connect_server

Connect a server to the current assistant or enable globally.

**Parameters:**

- `serverId` or `serverName` (string, required): Server to connect
- `scope` (string, optional): "assistant" or "global" (default: "assistant")
- `autoStart` (boolean, optional): Auto-start server (default: true, placeholder)

**Output:**

- Connection status with server details and scope

**Example:**

```typescript
// Connect to current assistant
await proxy.callTool('mcp_manager', 'connect_server', {
  serverName: 'my-github-server',
  scope: 'assistant',
});

// Enable globally for all assistants
await proxy.callTool('mcp_manager', 'connect_server', {
  serverId: 'mcp-1234567890-abc',
  scope: 'global',
});
```

### disconnect_server

Disconnect server from current assistant or disable globally.

**Parameters:**

- `serverId` or `serverName` (string, required)
- `scope` (string, optional): "assistant" or "global"

**Output:**

- Disconnection status

**Example:**

```typescript
await proxy.callTool('mcp_manager', 'disconnect_server', {
  serverName: 'my-github-server',
  scope: 'assistant',
});
```

## Context Integration

- Automatically receives `assistantId` via WebMCPContextSetter
- Assistant-scoped operations use current assistant context
- Global operations affect all assistants

## Response Format

All tools return both:

- **Text**: Human-readable summary with emojis (✅, ❌, 📋, 🔍, 💡)
- **Structured Content**: Machine-readable data for programmatic access

## Error Handling

- Duplicate name validation (case-insensitive)
- Transport configuration validation
- Assistant reference checking
- Clear error messages for all failure cases

## Limitations

- `autoStart` parameter is currently a placeholder (Tauri backend integration pending)
- Server health monitoring not yet implemented
- No update/delete operations (coming in Phase 2)
