# V1 Panel State Management Analysis

**Date**: 2024-12-30  
**Purpose**: Document how legacy Chat V1 panels retrieve structured state from built-in tools for Agent V2 implementation

---

## Executive Summary

The V1 system uses two distinct patterns for panel state management:

1. **Service Context Pattern** (`ChatPlanningPanel`): Automatic subscription to structured state from Web MCP servers
2. **Direct Backend Calls Pattern** (`WorkspaceFilesPanel`): Manual state management with explicit Rust backend calls

---

## 1. Service Context Pattern (ChatPlanningPanel)

### Architecture Flow

```
PlanningServer (Web MCP)
  ↓
  getServiceContext() → { contextPrompt, structuredState }
  ↓
BuiltInToolProvider.buildToolPrompt()
  ↓
  Collects all service contexts → serviceContexts[serviceId]
  ↓
useServiceContext<PlanningState>('planning')
  ↓
ChatPlanningPanel (Automatic UI Update)
```

### Key Components

#### 1. Service Interface (`BuiltInService`)

```typescript
// src/features/tools/types.ts
export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;

  // CRITICAL: Returns structured state for UI consumption
  getServiceContext: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;

  switchContext: (options?: ServiceContextOptions) => Promise<void>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
}

export interface ServiceContext<T = unknown> {
  contextPrompt: string; // For LLM system prompt
  structuredState?: T; // For UI components
}

export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string; // For multi-thread isolation
}
```

#### 2. Planning Server Implementation

```typescript
// src/lib/web-mcp/modules/planning-server/server.ts:273-320

async getServiceContext(
  options?: ServiceContextOptions,
): Promise<ServiceContext<PlanningState>> {
  const sessionId = options?.sessionId || 'default';
  const threadId = options?.threadId || sessionId;

  // 1. Ensure session is set
  stateManager.setSession(sessionId, threadId);

  // 2. Retrieve current state
  const state = await stateManager.getStateForSession(sessionId, threadId);

  // 3. Build context prompt for LLM
  const contextParts = ['## Planning'];
  if (state.goal) {
    contextParts.push(`\n**Current Goal:** "${state.goal}"`);
  } else {
    contextParts.push('\n**No Goal Set**');
  }

  // Build todos section, scratchpad, etc...

  // 4. Return both prompt and structured state
  return {
    contextPrompt: contextParts.join('\n'),
    structuredState: {
      goal: state.goal,
      lastClearedGoal: state.lastClearedGoal,
      todos: state.todos,
      scratchpad: state.scratchpad,
    },
  };
}
```

**State Structure**:

```typescript
// src/lib/web-mcp/modules/planning-server/types.ts
export interface PlanningState {
  goal: string | null;
  lastClearedGoal: string | null;
  todos: SimpleTodo[];
  scratchpad: ScratchpadItem[];
}

export interface SimpleTodo {
  id: number;
  title: string;
  description?: string;
  checked: boolean;
  priority?: 'low' | 'medium' | 'high';
  parentId?: number;
  subtasks?: SimpleTodo[]; // Nested structure
}

export interface ScratchpadItem {
  id: number;
  title?: string;
  content: string;
  tags?: string[];
  source?: string;
}
```

#### 3. BuiltInToolProvider - State Collection

```typescript
// src/features/tools/index.tsx:266-298

const buildToolPrompt = useCallback(async (): Promise<string> => {
  const prompts: string[] = [];
  const newServiceContexts: Record<string, unknown> = {};

  const currentSession = getCurrentSession();
  const contextOptions: ServiceContextOptions = {
    sessionId: currentSession?.id,
    assistantId: currentAssistant?.id,
    threadId: currentSession?.sessionThread.id,
  };

  // Collect service context prompts from all ready services
  if (currentSession?.id) {
    for (const [serviceId, entry] of serviceEntries.entries()) {
      if (entry.status === 'ready') {
        try {
          const result = await entry.service.getServiceContext(contextOptions);

          // 1. Add context prompt for LLM
          if (result.contextPrompt && result.contextPrompt.trim()) {
            prompts.push(result.contextPrompt);
          }

          // 2. Store structured state for UI
          if (result.structuredState !== undefined) {
            newServiceContexts[serviceId] = result.structuredState;
          }
        } catch (error) {
          logger.error('Failed to get service context', { serviceId, error });
        }
      }
    }
  }

  // Update state
  setServiceContexts(newServiceContexts);

  return prompts.join('\n\n');
}, [serviceEntries, getCurrentSession, currentAssistant]);
```

**Key Points**:

- Called during system prompt building (before LLM requests)
- Collects structured state from ALL ready services
- Stores in `serviceContexts` React state
- UI components subscribe via `useServiceContext`

#### 4. useServiceContext Hook

```typescript
// src/features/tools/useServiceContext.ts

export function useServiceContext<T = unknown>(
  serviceId: string,
): T | undefined {
  const { serviceContexts } = useBuiltInTool();
  return serviceContexts[serviceId] as T | undefined;
}
```

**Usage**:

```typescript
// src/features/chat/components/ChatPlanningPanel.tsx

export function ChatPlanningPanel() {
  // Automatic subscription to planning state
  const planningState = useServiceContext<PlanningState>('planning');

  // State is now automatically updated via service context
  // No manual API calls needed

  return (
    <Card>
      <CardContent>
        {/* Goal Section */}
        <div>{planningState?.goal || 'No active goal'}</div>

        {/* Todos Section */}
        {planningState?.todos.map((todo) => (
          <TodoItem key={todo.id} todo={todo} />
        ))}

        {/* Scratchpad Section */}
        {planningState?.scratchpad.map((item) => (
          <ScratchpadNote key={item.id} item={item} />
        ))}
      </CardContent>
    </Card>
  );
}
```

#### 5. Message-Triggered Updates

```typescript
// src/hooks/use-message-trigger.ts

export function useMessageTrigger(
  callback: () => void | Promise<void>,
  options: UseMessageTriggerOptions = {},
) {
  const { messages } = useChatState();
  const lastHandledRef = useRef<{ id?: string }>({});

  useEffect(() => {
    const lastMessage = messages[messages.length - 1];

    // Prevent duplicate processing
    if (lastHandledRef.current.id === lastMessage.id) return;

    lastHandledRef.current.id = lastMessage.id;

    // Apply debouncing
    setTimeout(() => callback(), options.debounceMs || 0);
  }, [messages, callback, options]);
}
```

**Usage in Panel**:

```typescript
// ChatPlanningPanel.tsx:24-28

useMessageTrigger(() => {
  // State is now automatically updated via useServiceContext
  // This hook just logs for debugging
  logger.debug('PLANNING_PANEL: State updated via service context');
});
```

**How It Works**:

- Watches `messages` array from `ChatContext`
- Triggers callback when new message arrives
- Debouncing prevents excessive updates
- Used to trigger `buildToolPrompt()` which refreshes `serviceContexts`

---

## 2. Direct Backend Calls Pattern (WorkspaceFilesPanel)

### Architecture Flow

```
User Action (expand folder, refresh)
  ↓
WorkspaceFilesPanel.loadDirectory(path)
  ↓
useRustBackend().listWorkspaceFiles(path)
  ↓
invoke('list_workspace_files', { path })
  ↓
Rust Backend → Returns WorkspaceFileItem[]
  ↓
setState(fileTree) → Manual UI Update
```

### Key Components

#### 1. useRustBackend Hook

```typescript
// src/hooks/use-rust-backend.ts (inferred interface)

export interface WorkspaceFileItem {
  name: string;
  path: string;
  isDirectory: boolean;
}

export function useRustBackend() {
  return {
    listWorkspaceFiles: (path: string) =>
      invoke<WorkspaceFileItem[]>('list_workspace_files', { path }),

    downloadWorkspaceFile: (path: string) =>
      invoke('download_workspace_file', { path }),

    callBuiltinTool: (server: string, tool: string, params: unknown) =>
      invoke('call_builtin_tool', { server, tool, params }),
  };
}
```

#### 2. Manual State Management

```typescript
// src/features/chat/components/WorkspaceFilesPanel.tsx:26-54

interface FileNode {
  id: string;
  name: string;
  path: string;
  isDirectory: boolean;
  children?: FileNode[];
  isExpanded?: boolean;
  isLoading?: boolean;
  parent?: string;
}

export function WorkspaceFilesPanel() {
  const { listWorkspaceFiles, downloadWorkspaceFile, callBuiltinTool } =
    useRustBackend();

  // Local state - NOT from service context
  const [rootPath, setRootPath] = useState<string>('./');
  const [fileTree, setFileTree] = useState<FileNode[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load directory on mount
  useEffect(() => {
    loadDirectory(rootPath);
  }, []);

  // Message-triggered refresh
  useMessageTrigger(() => {
    if (rootPath) {
      loadDirectory(rootPath);
    }
  }, { debounceMs: 100 });

  // Manual load function
  const loadDirectory = useCallback(async (path: string, parentNodeId?: string) => {
    setLoading(true);
    setError(null);

    try {
      const files = await listWorkspaceFiles(path);

      const nodes: FileNode[] = files.map((file) => ({
        id: `${path}/${file.name}`,
        name: file.name,
        path: `${path}/${file.name}`.replace('//', '/'),
        isDirectory: file.isDirectory,
        isExpanded: false,
        children: file.isDirectory ? [] : undefined,
        parent: parentNodeId,
      }));

      if (parentNodeId) {
        setFileTree((prev) => updateNodeChildren(prev, parentNodeId, nodes));
      } else {
        setFileTree(nodes);
      }
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  }, [listWorkspaceFiles]);

  // User-triggered actions
  const toggleDirectory = useCallback(async (node: FileNode) => {
    if (!node.isDirectory) return;

    if (node.isExpanded) {
      setFileTree((prev) => toggleNodeExpansion(prev, node.id, false));
    } else {
      setFileTree((prev) => toggleNodeExpansion(prev, node.id, true, true));
      await loadDirectory(node.path, node.id);
    }
  }, [loadDirectory]);

  return (
    <Card>
      <CardContent>
        {fileTree.map((node) => renderNode(node))}
      </CardContent>
    </Card>
  );
}
```

**Characteristics**:

- ❌ No `getServiceContext()` implementation
- ❌ No automatic state subscription
- ✅ Direct Rust backend calls via Tauri IPC
- ✅ Manual state management with React hooks
- ✅ Imperative API (call functions, update state)

#### 3. DnD Integration (File Drop)

```typescript
// WorkspaceFilesPanel.tsx:181-235

const handleWorkspaceFileDrop = useCallback(
  async (paths: string[]) => {
    if (!session?.id) return;

    try {
      for (const srcPath of paths) {
        const fileName = srcPath.split(/[/\\]/).pop() || 'unknown';
        const destPath = `${rootPath}/${fileName}`.replace(/\/+/g, '/');

        // Call builtin workspace tool
        const response = await callBuiltinTool('workspace', 'importFile', {
          src_abs_path: srcPath,
          dest_rel_path: destPath,
        });

        // Create tool messages for chat history
        const [toolCallMessage, toolResultMessage] = createToolMessagePair(
          'importFile',
          { src_abs_path: srcPath, dest_rel_path: destPath },
          stringToMCPContentArray(resultText),
          toolCallId,
          session.id,
        );

        await submit([toolCallMessage, toolResultMessage]);
      }

      // Refresh directory after import
      await loadDirectory(rootPath);
    } catch (error) {
      toast.error('Failed to import file');
    }
  },
  [callBuiltinTool, submit, session, rootPath, loadDirectory],
);

// Subscribe to DnD events
useEffect(() => {
  const handler = (event: DragAndDropEvent, payload: DragAndDropPayload) => {
    if (event === 'drag-over') {
      setDragState({ isOver: true });
    } else if (event === 'drop') {
      setDragState({ isOver: false });
      if (payload.paths) {
        handleWorkspaceFileDrop(payload.paths);
      }
    } else if (event === 'leave') {
      setDragState({ isOver: false });
    }
  };

  const unsub = subscribe(panelRef, handler, { priority: 5 });
  return () => unsub();
}, [subscribe, handleWorkspaceFileDrop]);
```

---

## 3. Pattern Comparison

| Aspect                | Service Context Pattern                | Direct Backend Pattern     |
| --------------------- | -------------------------------------- | -------------------------- |
| **Example**           | `ChatPlanningPanel`                    | `WorkspaceFilesPanel`      |
| **Backend**           | Web MCP Server (Worker)                | Rust Tauri Backend         |
| **State Source**      | `getServiceContext()`                  | Direct IPC calls           |
| **State Management**  | Automatic (context subscription)       | Manual (React state hooks) |
| **Update Trigger**    | `buildToolPrompt()` call               | Explicit function calls    |
| **State Structure**   | Defined TypeScript interface           | Ad-hoc local state         |
| **Session Isolation** | Built-in (via `ServiceContextOptions`) | Manual (pass sessionId)    |
| **Thread Support**    | Yes (via `threadId`)                   | No                         |
| **LLM Integration**   | Yes (contextPrompt)                    | No                         |
| **Complexity**        | Lower (declarative)                    | Higher (imperative)        |
| **Reusability**       | High (any service)                     | Low (component-specific)   |

---

## 4. Agent V2 Implementation Strategy

### For Planning-Like Features (Structured State)

**✅ Use Service Context Pattern When**:

- State is managed by Web MCP or Rust Built-in server
- State has defined TypeScript interface
- State needs to be in LLM context (system prompt)
- Multiple components need same state
- Session/thread isolation required

**Implementation Steps**:

1. **Create State Types**:

```typescript
// src/lib/web-mcp/modules/agent-planning-server/types.ts
export interface AgentPlanningState {
  currentGoal: string | null;
  activeTasks: AgentTask[];
  completedTasks: AgentTask[];
  scratchpad: Note[];
}
```

2. **Implement getServiceContext()**:

```typescript
// Agent planning server (Rust or Web MCP)
async getServiceContext(
  options?: ServiceContextOptions
): Promise<ServiceContext<AgentPlanningState>> {
  const state = await loadStateForSession(options?.sessionId);

  return {
    contextPrompt: buildPromptFromState(state),
    structuredState: state,
  };
}
```

3. **Create UI Component**:

```typescript
// src/features/agent/components/AgentPlanningPanel.tsx
export function AgentPlanningPanel() {
  const planningState = useServiceContext<AgentPlanningState>('agent_planning');

  useMessageTrigger(() => {
    // Automatic refresh on new messages
  });

  return (
    <Card>
      <h3>{planningState?.currentGoal || 'No goal'}</h3>
      {planningState?.activeTasks.map((task) => (
        <TaskCard key={task.id} task={task} />
      ))}
    </Card>
  );
}
```

### For Workspace-Like Features (Dynamic File System)

**✅ Use Direct Backend Pattern When**:

- State is highly dynamic (file system, network)
- State is too large for structured serialization
- State changes based on user actions (not LLM)
- No need for LLM context integration
- Performance-critical operations

**Implementation Steps**:

1. **Create Tauri Commands**:

```rust
// src-tauri/src/commands/agent_workspace.rs
#[tauri::command]
pub async fn agent_list_workspace_files(
    session_id: String,
    path: String,
) -> Result<Vec<FileItem>, String> {
    // Implementation
}
```

2. **Create React Hook**:

```typescript
// src/hooks/use-agent-backend.ts
export function useAgentBackend() {
  return {
    listFiles: (sessionId: string, path: string) =>
      invoke('agent_list_workspace_files', { sessionId, path }),

    addFile: (sessionId: string, path: string, content: string) =>
      invoke('agent_add_file', { sessionId, path, content }),
  };
}
```

3. **Create UI Component**:

```typescript
// src/features/agent/components/AgentWorkspacePanel.tsx
export function AgentWorkspacePanel() {
  const { currentSession } = useAgentSessionState();
  const { listFiles } = useAgentBackend();
  const [fileTree, setFileTree] = useState<FileNode[]>([]);

  const loadFiles = useCallback(async (path: string) => {
    const files = await listFiles(currentSession!.id, path);
    setFileTree(buildTree(files));
  }, [currentSession, listFiles]);

  useEffect(() => {
    loadFiles('./');
  }, [currentSession?.id]);

  return <FileTree nodes={fileTree} onExpand={loadFiles} />;
}
```

---

## 5. Critical Implementation Notes for Agent V2

### ⚠️ Service Context Updates Are NOT Automatic

**Common Misconception**:

> "useServiceContext subscribes to real-time updates from the server"

**Reality**:

```typescript
// serviceContexts is ONLY updated when buildToolPrompt() is called
const buildToolPrompt = useCallback(async () => {
  // This collects service contexts
  for (const [serviceId, entry] of serviceEntries.entries()) {
    const result = await entry.service.getServiceContext(contextOptions);
    newServiceContexts[serviceId] = result.structuredState;
  }

  setServiceContexts(newServiceContexts); // <- Single update point
}, [serviceEntries]);
```

**Update Triggers**:

1. Before LLM request (system prompt building)
2. Session context switch
3. Manual trigger via `useMessageTrigger` (common pattern)

**Implication for Agent V2**:

- Agent V2 uses Rust orchestration, which calls LLM via `llm:completion-request` event
- This means `buildToolPrompt()` is called before EACH LLM request
- Planning state will be fresh for each agent cycle
- ✅ Service Context Pattern works for Agent V2

### Session Isolation Requirements

**V1 Implementation**:

```typescript
// ChatPlanningContext uses global session from SessionContext
const { current: currentSession } = useSessionContext();

const contextOptions: ServiceContextOptions = {
  sessionId: currentSession?.id,
  assistantId: currentAssistant?.id,
  threadId: currentSession?.sessionThread.id,
};
```

**Agent V2 Requirements**:

```typescript
// Must use Agent V2 session system
const { currentSession } = useAgentSessionState();

const contextOptions: ServiceContextOptions = {
  sessionId: currentSession?.id,
  assistantId: agent?.id, // Different from V1 assistant
  threadId: currentSession?.id, // V2 uses sessionId as threadId
};
```

**Critical**:

- V1 and V2 sessions are SEPARATE systems
- Cannot share `SessionContext` (IndexedDB) vs `AgentSessionContext` (Rust/SQLite)
- Must create **separate** contexts: `AgentPlanningContext`, `AgentWorkspaceContext`

### Thread Isolation (Multi-Agent Support)

**V1 (Single Thread)**:

```typescript
// All tools share same thread per session
threadId: currentSession?.sessionThread.id; // Always same value
```

**V2 (Multi-Agent)**:

```typescript
// Each agent can have independent thread
export interface AgentSession {
  id: string;
  name?: string;
  agentIds: string[]; // Multiple agents possible
  threadIds: Map<string, string>; // Per-agent thread isolation
}

// When calling service context:
const agentThreadId =
  currentSession?.threadIds.get(agentId) || currentSession?.id;

const contextOptions: ServiceContextOptions = {
  sessionId: currentSession?.id,
  assistantId: agentId,
  threadId: agentThreadId, // Isolated per agent
};
```

---

## 6. Migration Checklist for Agent V2 Panels

### Planning Panel Migration

- [ ] Create `AgentPlanningContext.tsx` (separate from V1 `ChatPlanningContext`)
- [ ] Implement `useAgentPlanning()` hook
- [ ] Update planning server to support Agent V2 session IDs
- [ ] Verify thread isolation for multi-agent scenarios
- [ ] Create `AgentPlanningPanel.tsx` component
- [ ] Test state updates during agent workflow cycles
- [ ] Ensure no cross-contamination with V1 planning state

### Workspace Panel Migration

**Decision: Service Context vs Direct Backend?**

**Option A: Service Context Pattern (Recommended)**

- [ ] Implement `AgentWorkspaceServer` with `getServiceContext()`
- [ ] Define `AgentWorkspaceState` interface
- [ ] Create `useAgentWorkspace()` hook with `useServiceContext`
- [ ] Build `AgentWorkspacePanel.tsx` with automatic updates

**Option B: Direct Backend Pattern**

- [ ] Create Rust commands: `agent_list_files`, `agent_add_file`, etc.
- [ ] Create `useAgentBackend()` hook
- [ ] Build `AgentWorkspacePanel.tsx` with manual state management
- [ ] Implement DnD integration

**Recommendation**: Use Service Context Pattern for consistency, unless performance issues arise.

### Common Tasks for Both

- [ ] Create separate provider: `AgentPanelProvider`
- [ ] Implement panel toggle coordination (only one open at a time)
- [ ] Add panel state to AgentChatView header
- [ ] Test session switching doesn't leak state
- [ ] Verify panel state persists across UI remounts
- [ ] Add loading states and error handling
- [ ] Test with multiple concurrent agent sessions

---

## 7. Code References

### V1 Implementation Files

- Service Context: `src/features/tools/useServiceContext.ts`
- Provider: `src/features/tools/index.tsx`
- Planning Panel: `src/features/chat/components/ChatPlanningPanel.tsx`
- Workspace Panel: `src/features/chat/components/WorkspaceFilesPanel.tsx`
- Message Trigger: `src/hooks/use-message-trigger.ts`
- Planning Server: `src/lib/web-mcp/modules/planning-server/server.ts`

### V2 Target Files (To Create)

- Agent Planning Context: `src/context/AgentPlanningContext.tsx`
- Agent Workspace Context: `src/context/AgentWorkspaceContext.tsx`
- Agent Planning Panel: `src/features/agent/components/AgentPlanningPanel.tsx`
- Agent Workspace Panel: `src/features/agent/components/AgentWorkspacePanel.tsx`
- Agent Backend Hook: `src/hooks/use-agent-backend.ts`

---

**Status**: ✅ Analysis Complete - Ready for Agent V2 Implementation  
**Next Step**: Decide on Workspace Panel pattern (Service Context vs Direct Backend) and begin implementation
