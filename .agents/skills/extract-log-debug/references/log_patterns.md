# LibrAgent Log Patterns Reference

## Common Log Patterns

### Error Patterns

- `[ERROR]` - General errors
- `[error]` - Lowercase error markers
- `ERROR:` - Python/Rust error format
- `Error:` - Generic error messages
- `Failed to` - Operation failures
- `Cannot` / `Could not` - Capability errors
- `panic` - Rust panic traces

### Warning Patterns

- `[WARN]` - General warnings
- `[warn]` - Lowercase warning markers
- `WARN:` - Structured warning format
- `deprecated` - Deprecation warnings

### Debug Patterns

- `[DEBUG]` - Debug level logs
- `[debug]` - Lowercase debug markers
- `DEBUG:` - Structured debug format

### Component-Specific Patterns

#### Agent System

- `agent_create_session` - Session creation
- `agent_start_workflow` - Workflow start
- `agent_stop_workflow` - Workflow stop
- `AgentSessionManager` - Session manager logs
- `Think phase` / `Act phase` / `Observe phase` - Workflow phases

#### MCP Integration

- `MCPServiceProxy` - Service proxy operations
- `BuiltinMCPServer` - Builtin server logs
- `SessionMCPManager` - Session MCP manager
- `HttpSessionManager` - HTTP session manager
- `call_tool` - Tool execution
- `list_tools` - Tool listing

#### Planning System

- `PLANNING` - Planning module logs
- `create_goal` - Goal creation
- `add_todo` - Todo creation
- `complete_todo` - Todo completion

#### Browser Automation

- `BrowserServer` - Browser server logs
- `createSession` - Browser session creation
- `navigateToUrl` - Navigation operations
- `extractWebContent` - Content extraction

#### Workspace Operations

- `WorkspaceServer` - Workspace server logs
- `readFile` / `writeFile` / `editFile` - File operations
- `listFiles` - Directory listing
- `searchFiles` - File search

#### Knowledge Base

- `KnowledgeServer` - Knowledge server logs
- `add_knowledge` / `query_knowledge` - Knowledge operations

### Performance Patterns

- `Duration:` - Operation timing
- `took` - Duration indicators
- `elapsed` - Time elapsed
- `ms` / `seconds` - Time units

### LLM Integration

- `openai` - OpenAI provider
- `anthropic` - Anthropic provider
- `ollama` - Ollama provider
- `token` - Token usage
- `streaming` - Streaming responses
- `tool_calls` - Tool call parsing

## Search Strategies

### Debugging Workflow Issues

1. Search for session ID: `<session-id>`
2. Search for workflow phases: `Think phase`, `Act phase`, `Observe phase`
3. Search for tool execution: `call_tool`
4. Search for errors during session: `[ERROR]` with session context

### Debugging Tool Execution

1. Search for tool name: `<tool_name>`
2. Search for builtin server: `BuiltinMCPServer`
3. Search for MCPServiceProxy routing: `MCPServiceProxy`
4. Check for JSON parsing errors: `parse` or `JSON`

### Debugging Planning System

1. Search for planning operations: `PLANNING`
2. Search for specific operations: `create_goal`, `add_todo`, `complete_todo`
3. Check database operations: `SeaORM` or `database`

### Debugging Browser Automation

1. Search for session creation: `createSession`
2. Search for navigation: `navigateToUrl`
3. Search for interaction: `clickElement`, `listInteractable`
4. Check for CDP errors: `Chrome DevTools Protocol`

## Context Extraction Tips

- Use 5-10 lines of context for errors to see preceding operations
- Use 20-50 lines for workflow analysis to see full sequences
- For performance issues, search for duration patterns with wide context
- For database issues, search for `SeaORM` with context to see queries
