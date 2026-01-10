# Agent V2 BuiltInToolProvider Implementation Plan

**Date**: 2026-01-10  
**Phase**: 0 - Architecture Preparation  
**Goal**: Create session-scoped BuiltInToolProvider for Agent V2

---

## 1. Architecture Overview

### Current (Legacy) Architecture

```
App (Global)
└─ BuiltInToolProvider (Shared, App-scoped)
   ├─ Uses: SessionContext.current ← V1 pattern
   ├─ Uses: AssistantContext.currentAssistant ← V1 pattern
   └─ Effect watches "current session" → switchContext()

/chat/single (V1) → Uses global BuiltInToolProvider
/agent/:sessionId (V2) → Uses global BuiltInToolProvider ❌ BROKEN
```

**Problem**: V2 sessions don't set "current session", so tools never switch context!

### New (V2) Architecture

```
App (Global)
├─ BuiltInToolProvider (Legacy, for V1 only)
│
└─ /agent/:sessionId (V2)
   └─ AgentSessionProvider (sessionId prop)
      ├─ AgentBuiltInToolProvider (NEW, session-scoped) ✅
      │  └─ Provides tools for THIS session only
      └─ AgentChatView
```

**Benefits**:

- ✅ Each V2 session has isolated tool context
- ✅ Tools automatically know their session ID
- ✅ No global "current session" dependency
- ✅ Supports parallel sessions (future: tabs)

---

## 2. Implementation Tasks

### 2.1 Create AgentBuiltInToolContext

**File**: `src/features/agent/contexts/AgentBuiltInToolContext.tsx`

**Interface Design**:

```typescript
interface AgentBuiltInToolContextType {
  // Session info (from parent AgentSessionProvider)
  sessionId: string;
  assistant: Assistant | undefined;

  // Tool management
  register: (serviceId: string, service: BuiltInService) => void;
  unregister: (serviceId: string) => void;
  availableTools: MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  buildToolPrompt: () => Promise<string>;
  status: Record<string, ServiceStatus>;
  getServiceMetadata: (alias: string) => ServiceMetadata | null;
  serviceContexts: Record<string, unknown>;
}

interface AgentBuiltInToolProviderProps {
  children: ReactNode;
}
```

**Key Differences from Legacy**:

1. ❌ No `useSessionContext()` dependency
2. ❌ No `useAssistantContext()` dependency
3. ✅ Gets session info from parent `AgentSessionContext`
4. ✅ No global "current" tracking
5. ✅ Auto-switches context on mount (once per session)

**Implementation Strategy**:

```typescript
export function AgentBuiltInToolProvider({
  children,
}: AgentBuiltInToolProviderProps) {
  const { session } = useAgentSessionState(); // Get session from parent
  const { register: registerPrompt, unregister: unregisterPrompt } =
    useSystemPrompt();

  // Services registry (same as legacy)
  const [serviceEntries, setServiceEntries] = useState<
    Map<string, ServiceEntry>
  >(new Map());
  const [serviceContexts, setServiceContexts] = useState<
    Record<string, unknown>
  >({});

  // Switch context ONCE on mount (not on every session.id change)
  useEffect(() => {
    const sessionId = session?.id;
    const assistantId = session?.assistant?.id;

    if (!sessionId) return;

    logger.info('AgentBuiltInToolProvider: Initializing for session', {
      sessionId,
    });

    // Call Rust backend to switch session
    switchSession(sessionId, true).catch((error) => {
      logger.error('Failed to switch session in backend', { sessionId, error });
    });

    // Switch context on all registered services
    const readyServices = Array.from(serviceEntriesRef.current.values()).filter(
      (e) => e.status === 'ready',
    );

    if (readyServices.length > 0) {
      Promise.allSettled(
        readyServices.map((entry) =>
          entry.service.switchContext({ sessionId, assistantId }),
        ),
      ).then((results) => {
        const failures = results.filter((r) => r.status === 'rejected');
        if (failures.length > 0) {
          logger.warn('Some services failed to switch context', {
            total: results.length,
            failed: failures.length,
          });
        }
      });
    }
  }, []); // ✅ Empty deps - run ONCE on mount

  // Rest of implementation same as legacy...
}
```

### 2.2 Integration into AgentSessionProvider

**File**: `src/context/AgentSessionContext.tsx`

**Modification**:

```typescript
export function AgentSessionProvider({ children, sessionId }: AgentSessionProviderProps) {
  // ... existing state management ...

  return (
    <AgentSessionStateContext.Provider value={stateValue}>
      <AgentSessionActionsContext.Provider value={actionsValue}>
        <AgentBuiltInToolProvider>  {/* ✅ NEW: Wrap children */}
          {children}
        </AgentBuiltInToolProvider>
      </AgentSessionActionsContext.Provider>
    </AgentSessionStateContext.Provider>
  );
}
```

**Alternative**: Keep as separate in AgentChatView (more explicit)

```typescript
// src/features/agent/AgentChatView.tsx
export default function AgentChatView() {
  const { session } = useAgentSessionState();

  return (
    <AgentChatProvider>
      <AgentBuiltInToolProvider>  {/* ✅ Session-scoped */}
        <div className="h-full flex flex-col">
          <AgentChat.Header />
          <AgentChat.Messages />
          <AgentChat.Bottom />
        </div>
      </AgentBuiltInToolProvider>
    </AgentChatProvider>
  );
}
```

**Decision**: Use Alternative (more explicit, easier to understand hierarchy)

### 2.3 Service Registration

**Decision: V2 Does NOT Need Frontend Service Registration**

After analysis, V2 Agent uses a completely different tool architecture:

- **V1 Chat**: Uses WebMCPServiceRegistry, BrowserToolProvider, RustMCPToolProvider (frontend-based)
- **V2 Agent**: Uses Rust builtin servers directly via MCPServiceProxy (backend-managed, session-scoped)

**No changes needed** to:

- `src/features/tools/WebMCPServiceRegistry.tsx` (V1-only)
- `src/features/tools/BrowserToolProvider.tsx` (V1-only)
- `src/features/tools/RustMCPToolProvider.tsx` (V1-only)

These components remain V1-only and will be removed in later phases when V1 Chat is removed.

---

## 3. Testing Strategy

### 3.1 Unit Tests

- [ ] Tool registration works
- [ ] Service status tracking works
- [ ] Tool execution with session context
- [ ] Context switching on mount
- [ ] Multiple services registered simultaneously

### 3.2 Integration Tests

- [ ] Create Agent V2 session
- [ ] Verify tools are available
- [ ] Execute planning tool
- [ ] Execute browser tool
- [ ] Execute knowledge tool
- [ ] Verify tool results display correctly

### 3.3 Isolation Tests

- [ ] Create two sessions in different routes
- [ ] Verify each session has isolated tool context
- [ ] Execute same tool in both sessions
- [ ] Verify results don't cross-contaminate

### 3.4 Backward Compatibility Tests

- [ ] V1 chat still works (uses legacy provider)
- [ ] V2 agent still works (uses new provider)
- [ ] No interference between V1 and V2

---

## 4. Migration Path

### Phase 0.1: Create New Provider

- Create `AgentBuiltInToolContext.tsx`
- Implement core functionality (copy from legacy)
- Remove SessionContext/AssistantContext dependencies
- Add session-scoped context switching

### Phase 0.2: Integrate into V2

- Add to `AgentChatView.tsx` (not `AgentSessionProvider`)
- Update service registration to support both contexts
- Test V2 tools work

### Phase 0.3: Verify Isolation

- Test multiple V2 sessions
- Verify no V2→V1 dependencies
- Document architecture changes

### Phase 0.4: Ready for V1 Removal

- All V2 functionality confirmed working
- No shared state between V1 and V2
- Clean separation of concerns

---

## 5. Code Structure

### New Files

```
src/features/agent/
├─ contexts/
│  └─ AgentBuiltInToolContext.tsx    # NEW: Session-scoped tool provider
└─ AgentChatView.tsx                 # MODIFIED: Wrap with provider
```

### Modified Files

```
src/hooks/
└─ use-builtin-tool.ts               # MODIFIED: Support both contexts

src/features/tools/
├─ WebMCPServiceRegistry.tsx         # MODIFIED: Use new hook
├─ BrowserToolProvider.tsx           # MODIFIED: Use new hook
└─ RustMCPToolProvider.tsx           # MODIFIED: Use new hook
```

### Unchanged (Legacy)

```
src/features/tools/
└─ index.tsx                         # BuiltInToolProvider (legacy, for V1)
```

---

## 6. Success Criteria

- [ ] ✅ Agent V2 sessions have isolated tool contexts
- [ ] ✅ Tools execute with correct session ID
- [ ] ✅ No SessionContext dependency in V2
- [ ] ✅ No AssistantContext dependency in V2
- [ ] ✅ Planning tool works in V2
- [ ] ✅ Browser tool works in V2
- [ ] ✅ Knowledge tool works in V2
- [ ] ✅ Multiple V2 sessions don't interfere
- [ ] ✅ V1 chat still works (backward compatible)
- [ ] ✅ No V2→V1 code dependencies

---

## 7. Risk Mitigation

### Risk: Service registration breaks

**Mitigation**: Dual hook pattern maintains backward compatibility

### Risk: Tool context not switching

**Mitigation**: Explicit logging + manual testing

### Risk: V1 breaks during migration

**Mitigation**: Keep legacy provider untouched

### Risk: Incomplete isolation

**Mitigation**: Test parallel sessions thoroughly

---

## 8. Next Steps

After Phase 0 completion:

1. **Phase 1**: Verify no V2→legacy dependencies
2. **Phase 2**: Remove V1 routes and navigation
3. **Phase 3**: Remove V1 chat feature
4. **Phase 4**: Remove legacy BuiltInToolProvider
5. **Phase 5**: Final validation and documentation

---

**Estimated Time**: 4-6 hours  
**Complexity**: Medium-High  
**Risk Level**: Medium (backward compatibility managed)
