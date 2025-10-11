# Multi-Agent Orchestration Master Plan

## 📋 Overview

Enable assistants to discover, spawn, and coordinate with other assistants through a built-in MCP server and lightweight runner architecture.

## 🎯 Goals

1. **Assistant Discovery**: Search and list available assistants
2. **Task Delegation**: Spawn assistants for sub-tasks
3. **Inter-Process Communication**: Message box for assistant coordination
4. **State Management**: Track and monitor spawned assistant processes

## 🏗️ Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Main Process (Tauri)                     │
├─────────────────────────────────────────────────────────────┤
│  ┌────────────────┐  ┌──────────────────────────────────┐  │
│  │ MCPServerMgr   │  │  Built-in: assistant             │  │
│  │                │──│  - list_assistant                │  │
│  │ - External MCP │  │  - search_assistant (BM25)       │  │
│  │ - Built-in MCP │  │  - spawn_assistant               │  │
│  │                │  │  - poll_assistant                │  │
│  └────────────────┘  │  - send_message                  │  │
│                      │  - recv_message                  │  │
│  ┌────────────────┐  └──────────────────────────────────┘  │
│  │ SQLite Storage │                                         │
│  │ - assistants   │  ┌──────────────────────────────────┐  │
│  │ - processes    │  │  RunnerManager                   │  │
│  │ - messages     │  │  - Spawn stdio runners           │  │
│  │ - search index │  │  - RPC communication             │  │
│  └────────────────┘  │  - Lifecycle management          │  │
│                      └──────────────────────────────────┘  │
└───────────────────────────┬─────────────────────────────────┘
                            │ stdio JSON-RPC
                ┌───────────┴───────────┐
                │                       │
        ┌───────▼────────┐      ┌──────▼──────────┐
        │ Runner #1      │      │ Runner #2       │
        │ Thread: t-001  │      │ Thread: t-002   │
        ├────────────────┤      ├─────────────────┤
        │ - LLM Proxy    │      │ - LLM Proxy     │
        │ - Tool Handler │      │ - Tool Handler  │
        │ - Message Box  │      │ - Message Box   │
        └────────────────┘      └─────────────────┘
```

## 📊 Data Model

### SQLite Schema

```sql
-- Assistant Registry (Source of Truth)
CREATE TABLE assistants (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT,
  system_prompt TEXT NOT NULL,
  mcp_config TEXT NOT NULL,          -- JSON
  allowed_builtin_service_aliases TEXT, -- JSON array
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

-- BM25 Full-Text Search
CREATE VIRTUAL TABLE assistant_search
  USING fts5(id, name, description);

-- Process State Tracking
CREATE TABLE assistant_processes (
  process_id TEXT PRIMARY KEY,
  assistant_id TEXT NOT NULL,
  parent_assistant_id TEXT,          -- Who spawned this
  session_id TEXT NOT NULL,
  thread_id TEXT NOT NULL UNIQUE,    -- Unique execution context
  status TEXT NOT NULL,              -- starting|running|finished|failed
  query TEXT NOT NULL,               -- Initial task
  started_at INTEGER NOT NULL,
  finished_at INTEGER,
  exit_code INTEGER,
  FOREIGN KEY (assistant_id) REFERENCES assistants(id)
);

-- Inter-Assistant Message Box
CREATE TABLE assistant_messages (
  id TEXT PRIMARY KEY,
  thread_id TEXT NOT NULL,
  from_assistant_id TEXT NOT NULL,
  to_assistant_id TEXT NOT NULL,
  content TEXT NOT NULL,              -- Message content (text or JSON)
  created_at INTEGER NOT NULL,
  read_at INTEGER,                    -- NULL = unread
  INDEX idx_thread_created (thread_id, created_at),
  INDEX idx_thread_unread (thread_id, read_at)  -- Optimize unread queries
);
```

## 🛠️ Built-in MCP Tools

### 1. list_assistant

```typescript
{
  inputSchema: {
    page: number = 1,
    pageSize: number = 20
  },
  returns: {
    items: Assistant[],
    totalItems: number
  }
}
```

### 2. search_assistant

```typescript
{
  inputSchema: {
    query: string,              // BM25 search query
    limit: number = 10
  },
  returns: Assistant[]
}
```

### 3. spawn_assistant

```typescript
{
  inputSchema: {
    assistant_id: string,
    query: string,              // Task description
    run_mode: "async" | "sync" = "async"
  },
  returns: {
    process_id: string,
    thread_id: string,
    status: "starting"
  }
}
```

### 4. poll_assistant

**Purpose**: Lightweight status check + message summary (monitoring only)

```typescript
{
  inputSchema: {
    process_id: string,
    include_summary: boolean = true  // Include message box summary
  },
  returns: {
    process: {
      process_id: string,
      assistant_id: string,
      thread_id: string,
      status: "starting" | "running" | "finished" | "failed",
      started_at: number,
      finished_at?: number,
      exit_code?: number
    },
    message_summary?: {
      total_messages: number,
      unread_messages: number,      // Key metric for polling
      last_message_at: number | null // Last message timestamp
    }
  }
}
```

**Design Notes**:

- Optimized for frequent polling (every 5-10 seconds)
- Returns only counts and timestamps (no message content)
- Use `recv_message` to fetch actual message content

### 5. send_message

```typescript
{
  inputSchema: {
    to_assistant_id: string,
    thread_id: string,
    message: string
  },
  returns: { success: boolean }
}
```

### 6. recv_message

**Purpose**: Fetch actual message content with filtering and pagination

```typescript
{
  inputSchema: {
    thread_id: string,

    // Filtering options
    unread_only: boolean = false,     // Fetch only unread messages
    last_n: number = 20,              // Recent N messages (max 200)
    since: number,                    // Unix timestamp (ms) - messages after this time

    // Read management
    mark_as_read: boolean = false     // Mark fetched messages as read
  },
  returns: {
    thread_id: string,
    messages: [{
      id: string,
      from_assistant_id: string,
      to_assistant_id: string,
      content: string,                // Message content
      created_at: number,
      read_at: number | null          // NULL if unread
    }],
    summary: {
      total_fetched: number,
      marked_as_read: number          // Count of messages marked as read
    }
  }
}
```

**Design Notes**:

- All message fetching logic concentrated here
- Supports multiple filtering strategies (unread, time range, last N)
- Transactional `mark_as_read` operation
- Default limit (20) prevents excessive data transfer

## 🔄 Runner RPC Protocol

### Runner → Main Process

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "on_llm_request",
  "params": {
    "thread_id": "t-001",
    "messages": [...]
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "on_tool_call",
  "params": {
    "thread_id": "t-001",
    "tool_name": "builtin_workspace__execute_shell",
    "arguments": {...}
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "on_aggregate_service_context",
  "params": {
    "thread_id": "t-001"
  }
}
```

### Main Process → Runner

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "role": "assistant",
    "content": "...",
    "tool_calls": [...]
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{ "type": "text", "text": "Tool result" }]
  }
}
```

## 📁 File Structure

```text
src-tauri/src/mcp/builtin/assistant/
├── mod.rs                    # AssistantServer + BuiltinMCPServer impl
├── storage.rs                # SQLite CRUD + BM25 indexing
├── runner_manager.rs         # Process spawning & lifecycle
├── message_box.rs            # Inter-assistant messaging
└── tools/
    ├── mod.rs
    ├── search_tools.rs       # list, search
    ├── spawn_tools.rs        # spawn
    └── poll_tools.rs         # poll, send_message, recv_message

src-tauri/bin/assistant-runner/
├── main.rs                   # Entry point, stdio loop
├── rpc.rs                    # JSON-RPC protocol handler
├── tool_handler.rs           # Built-in tool dispatcher
├── llm_proxy.rs              # LLM request proxy
└── message_box_client.rs     # Message box RPC client
```

## 🔧 Integration Points

### 1. SessionManager Integration

- Runner processes tied to session lifecycle
- Cleanup on session switch (like ProcessRegistry)

### 2. ProcessRegistry Pattern Reuse

- Similar to `workspace` server's process management
- Shared cancellation token pattern
- Output file handling (stdout/stderr)

### 3. get_service_context

```rust
impl BuiltinMCPServer for AssistantServer {
    fn get_service_context(&self, options: Option<&Value>) -> ServiceContext {
        let running = self.runner_manager.list_running();
        let pending_msgs = self.message_box.count_pending();

        ServiceContext {
            context_prompt: format!(
                "# Assistant Orchestration\n\
                - Running: {}\n\
                - Pending Messages: {}",
                running.len(), pending_msgs
            ),
            structured_state: Some(json!({ running, pending_msgs }))
        }
    }
}
```

## 📝 Implementation Phases

### Phase 1: Storage Foundation (2 days)

- [ ] Add SQLite tables for assistants/processes/messages
- [ ] Implement BM25 FTS5 indexing
- [ ] Create Tauri commands for assistant CRUD
- [ ] Frontend ↔ Backend sync logic

### Phase 2: Built-in Assistant Server (3 days)

- [ ] Create `builtin/assistant/` module structure
- [ ] Implement `list_assistant`, `search_assistant`
- [ ] Implement `spawn_assistant` + process registry
- [ ] Basic message box storage

### Phase 3: Lightweight Runner (4 days)

- [ ] Create `bin/assistant-runner` binary
- [ ] Implement stdio JSON-RPC protocol
- [ ] LLM request proxy (delegate to main process)
- [ ] Built-in tool handler (reuse existing tools)
- [ ] Message box client

### Phase 4: Message Box & Coordination (3 days)

- [ ] Complete `send_message`, `recv_message` tools
- [ ] Thread-based message isolation
- [ ] Parent-child relationship management
- [ ] `poll_assistant` with message retrieval

### Phase 5: Testing & Integration (2 days)

- [ ] End-to-end spawn → execute → poll flow
- [ ] Multi-assistant coordination scenarios
- [ ] Session cleanup integration
- [ ] Performance optimization

## 🎪 Usage Example

```typescript
// 1. Discover available assistants
const assistants = await callTool('builtin_assistant__search_assistant', {
  query: 'python code analyzer',
});

// 2. Spawn specialist assistant
const { process_id, thread_id } = await callTool(
  'builtin_assistant__spawn_assistant',
  {
    assistant_id: assistants[0].id,
    query: 'Analyze this Python code for bugs',
    run_mode: 'async',
  },
);

// 3. Lightweight status check (polling pattern)
const status = await callTool('builtin_assistant__poll_assistant', {
  process_id,
});

console.log(`Status: ${status.process.status}`);
console.log(`Unread messages: ${status.message_summary.unread_messages}`);

// 4. Send coordination message
await callTool('builtin_assistant__send_message', {
  to_assistant_id: assistants[0].id,
  thread_id,
  message: 'Focus on security vulnerabilities',
});

// 5. Fetch messages only when needed
if (status.message_summary.unread_messages > 0) {
  const messages = await callTool('builtin_assistant__recv_message', {
    thread_id,
    unread_only: true,
    mark_as_read: true,
  });

  messages.messages.forEach((msg) => {
    console.log(`${msg.from_assistant_id}: ${msg.content}`);
  });
}
```

## ⚠️ Key Decisions

| Decision           | Choice                               | Rationale                                      |
| ------------------ | ------------------------------------ | ---------------------------------------------- |
| Storage            | SQLite (Backend) + IndexedDB (Cache) | Multi-process support, BM25 search             |
| Runner             | Rust binary (stdio RPC)              | Lightweight, isolated, type-safe               |
| Message Box        | SQLite table                         | Persistent, debuggable, transactional          |
| Process Management | Reuse ProcessRegistry pattern        | Proven session-based cleanup                   |
| Service Context    | Include runner/message stats         | LLM visibility into orchestration state        |
| API Separation     | poll (status) vs recv (messages)     | Clear responsibility, performance optimization |

## 🔐 Security Considerations

1. **Process Isolation**: Each runner has unique thread_id
2. **Message Access Control**: Only parent/child/siblings share message box
3. **Resource Limits**: Max concurrent runners per session
4. **Timeout Management**: Auto-terminate stale runners (24h)

## 📈 Future Enhancements

- [ ] Assistant capability tags for smart matching
- [ ] Cost tracking per assistant execution
- [ ] Visual orchestration graph in UI
- [ ] Assistant templates marketplace
- [ ] Streaming updates from runners (SSE)
