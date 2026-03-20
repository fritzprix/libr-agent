import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    AgentSessionListProvider,
    useAgentSessionListState,
    useAgentSessionListActions,
} from '../AgentSessionListContext';
import { safeInvoke } from '@/lib/backend/core';
import { listen } from '@tauri-apps/api/event';
import type { Assistant } from '@/models/chat';
import { MemoryRouter } from 'react-router-dom';

// Mock Assistant for creating sessions
const mockAssistant: Assistant = {
    id: 'asst-1',
    name: 'Assistant',
    systemPrompt: 'sys prompt',
    deletionProtected: false,
    createdAt: new Date(),
    updatedAt: new Date(),
};

// Mock Tauri APIs
vi.mock('@/lib/backend/core', () => ({
    safeInvoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
    getLogger: () => ({
        info: vi.fn(),
        debug: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
    }),
}));

// Mock ModelProvider
vi.mock('../ModelProvider', () => ({
    useModelOptions: () => ({
        modelId: 'test-model',
        provider: 'test-provider',
    }),
}));

// Mock GlobalEventContext
vi.mock('../GlobalEventContext', () => ({
    useBackendResource: vi.fn(),
}));

vi.mock('@/context/SettingsContext', () => ({
    useSettings: () => ({
        value: {
            advanced: {
                defaultSessionMaxDepth: 0,
                defaultSessionMaxFanout: 0,
            },
        },
    }),
}));

vi.mock('../LLMServiceContext', () => ({
    useLLMService: () => ({
        clearSessionState: vi.fn(),
        clearAllCompactState: vi.fn(),
    }),
}));

function TestWrapper({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

describe('AgentSessionListContext', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        // Default: listen resolves to an unlisten function
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
    });

    it('should provide initial state', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapper,
        });

        expect(result.current.sessions).toEqual([]);
        // It might be loading immediately due to useEffect
        // So we wait for it to settle (mock returns empty array instantly)

        await waitFor(() => {
            expect(result.current.isSessionsListLoading).toBe(false);
        });
    });

    it('should load sessions on mount', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([
            {
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            }
        ]);

        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapper,
        });

        await waitFor(() => {
            expect(result.current.sessions).toHaveLength(1);
            expect(result.current.sessions[0].id).toBe('session-1');
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_get_all_sessions');
    });

    it('should create a new session', async () => {
        // Mock create response
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([]);
            if (cmd === 'get_assistant') {
                return Promise.resolve({
                    id: args.id,
                    name: 'Test Assistant',
                    config: JSON.stringify({
                        systemPrompt: 'You are a test assistant.',
                        allowedBuiltInServiceAliases: ['planning', 'knowledge']
                    }),
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                });
            }
            if (cmd === 'agent_create_session') {
                return Promise.resolve({
                    id: args.request.sessionId,
                    name: args.request.name,
                    status: 'idle',
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                });
            }
            return Promise.reject(new Error(`Unknown command ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper }
        );

        await act(async () => {
            await result.current.actions.createSession({
                name: 'New Session',
                assistant: mockAssistant,
            });
        });

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
            expect(result.current.state.sessions[0].name).toBe('New Session');
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_create_session', expect.anything());
    });

    it('should delete a session', async () => {
        // Setup: Load one session
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([
                {
                    id: 'session-1',
                    name: 'Test Session',
                    status: 'idle',
                    createdAt: Date.now(),
                }
            ]);
            if (cmd === 'agent_delete_session') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper }
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
        });

        await act(async () => {
            await result.current.actions.deleteSession('session-1');
        });

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(0);
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'session-1' });
    });
});

// ---------------------------------------------------------------------------
// Crash-recovery: statusChanged event patches session list in-place
// ---------------------------------------------------------------------------
describe('AgentSessionListContext – statusChanged event (crash recovery)', () => {
    const mockUnlisten = vi.fn();

    // Capture the agent:event handler registered by the useEffect listen() call
    let agentEventHandler: ((event: { payload: unknown }) => void) | undefined;

    beforeEach(() => {
        vi.clearAllMocks();
        agentEventHandler = undefined;

        (listen as ReturnType<typeof vi.fn>).mockImplementation(
            async (eventName: string, handler: (event: { payload: unknown }) => void) => {
                if (eventName === 'agent:event') {
                    agentEventHandler = handler;
                }
                return mockUnlisten;
            },
        );

        // Return a single existing session (paused – simulates crash-recovered child)
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([
            {
                id: 'session-child',
                name: 'Child Session',
                status: 'paused',
                model: 'gpt-4o',
                provider: 'openai',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            },
        ]);
    });

    function TestWrapperWithEvent({ children }: { children: React.ReactNode }) {
        return (
            <MemoryRouter>
                <AgentSessionListProvider>{children}</AgentSessionListProvider>
            </MemoryRouter>
        );
    }

    it('registers an agent:event listener on mount', async () => {
        renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(listen).toHaveBeenCalledWith('agent:event', expect.any(Function));
        });
    });

    it('patches session status to "busy" in-place when statusChanged fires', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        // Wait for initial load (sessions=[paused child])
        await waitFor(() => {
            expect(result.current.sessions).toHaveLength(1);
            expect(result.current.sessions[0].status).toBe('paused');
            expect(agentEventHandler).toBeDefined();
        });

        // Simulate Rust emitting StatusChanged { sessionId: 'session-child', status: 'busy' }
        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'statusChanged',
                    sessionId: 'session-child',
                    status: 'busy',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.sessions[0].status).toBe('busy');
        });
    });

    it('patches "busy" → "paused" correctly (e.g. intentional pause)', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([
            {
                id: 'session-x',
                name: 'Running Session',
                status: 'busy',
                model: 'gpt-4o',
                provider: 'openai',
                createdAt: Date.now(),
            },
        ]);

        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(result.current.sessions[0].status).toBe('busy');
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: { type: 'statusChanged', sessionId: 'session-x', status: 'paused' },
            });
        });

        await waitFor(() => {
            expect(result.current.sessions[0].status).toBe('paused');
        });
    });

    it('does NOT reload sessions (invoke) when statusChanged fires', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
        });

        const invokeCallsBefore = (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.length;

        act(() => {
            agentEventHandler?.({
                payload: { type: 'statusChanged', sessionId: 'session-child', status: 'busy' },
            });
        });

        // Flush microtasks – no new invoke calls expected
        await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
        });

        expect((safeInvoke as ReturnType<typeof vi.fn>).mock.calls.length).toBe(invokeCallsBefore);

        // Status IS updated without a reload
        expect(result.current.sessions[0].status).toBe('busy');
    });

    it('ignores statusChanged for an unknown session ID', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(result.current.sessions).toHaveLength(1);
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'statusChanged',
                    sessionId: 'session-UNKNOWN',
                    status: 'busy',
                },
            });
        });

        // Original session status must remain unchanged
        expect(result.current.sessions[0].status).toBe('paused');
        expect(result.current.sessions[0].id).toBe('session-child');
    });

    it('ignores non-statusChanged event types', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(result.current.sessions).toHaveLength(1);
            expect(result.current.sessions[0].status).toBe('paused');
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: { type: 'workflowStarted', sessionId: 'session-child' },
            });
        });

        expect(result.current.sessions[0].status).toBe('paused');
    });

    it('does not create a notification for a plain messageAdded event', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
            expect(result.current.notificationSessions).toHaveLength(0);
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'messageAdded',
                    sessionId: 'session-child',
                    message: {
                        role: 'assistant',
                        createdAt: Date.now(),
                    },
                },
            });
        });

        await waitFor(() => {
            expect(result.current.unreadNotificationCount).toBe(0);
            expect(result.current.notificationSessions).toHaveLength(0);
        });
    });

    it('tracks notifications when an inactive session hits a recurring stop condition', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
            expect(result.current.notificationSessions).toHaveLength(0);
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'workflowCompleted',
                    sessionId: 'session-child',
                    reason: 'recurringStop',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.unreadNotificationCount).toBe(1);
            expect(result.current.notificationSessions[0]?.id).toBe('session-child');
            expect(result.current.notificationSessions[0]?.lastAttentionReason).toBe('recurringStop');
        });
    });

    it('clears recurring-stop notifications when the session is marked viewed', async () => {
        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
        );

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'workflowCompleted',
                    sessionId: 'session-child',
                    reason: 'recurringStop',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.state.unreadNotificationCount).toBe(1);
        });

        await act(async () => {
            await result.current.actions.markSessionViewed(
                'session-child',
                new Date(Date.now() + 1000),
            );
        });

        await waitFor(() => {
            expect(result.current.state.unreadNotificationCount).toBe(0);
            expect(result.current.state.notificationSessions).toHaveLength(0);
        });
    });

    it('ignores natural workflow completion for notifications', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'workflowCompleted',
                    sessionId: 'session-child',
                    reason: 'natural',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.unreadNotificationCount).toBe(0);
        });
    });

    it('deduplicates repeated approval events for the same tool call', async () => {
        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
        );

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'toolExecutionRequiresApproval',
                    sessionId: 'session-child',
                    toolCallId: 'call-1',
                },
            });
            agentEventHandler?.({
                payload: {
                    type: 'toolExecutionRequiresApproval',
                    sessionId: 'session-child',
                    toolCallId: 'call-1',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.state.sessions[0]?.pendingApprovalCount).toBe(1);
        });

        act(() => {
            result.current.actions.clearPendingApproval('session-child', 'call-1');
        });

        expect(result.current.state.sessions[0]?.pendingApprovalCount).toBe(0);
    });

    it('unregisters the listener on unmount', async () => {
        const { unmount } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

        await waitFor(() => {
            expect(agentEventHandler).toBeDefined();
        });

        unmount();

        // The unlisten function returned by listen() must have been called
        expect(mockUnlisten).toHaveBeenCalled();
    });
});

// ---------------------------------------------------------------------------
// SP7: Session delete options – cascade delete vs. delete-only (orphan)
// ---------------------------------------------------------------------------
describe('AgentSessionListContext – SP7 session delete options', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
    });

    function TestWrapper({ children }: { children: React.ReactNode }) {
        return (
            <MemoryRouter>
                <AgentSessionListProvider>{children}</AgentSessionListProvider>
            </MemoryRouter>
        );
    }

    // ── deleteSession (cascade) ──────────────────────────────────────────────

    it('deleteSession: BFS removes parent AND direct child from UI', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'parent', name: 'Parent', status: 'idle', createdAt: Date.now() },
                    { id: 'child', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'parent' },
                ]);
            if (cmd === 'agent_delete_session') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        await act(async () => {
            await result.current.actions.deleteSession('parent');
        });

        // Both parent and child must be removed
        await waitFor(() => expect(result.current.state.sessions).toHaveLength(0));
        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'parent' });
    });

    it('deleteSession: BFS removes entire 3-level tree (grandparent → parent → child)', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'gp', name: 'Grandparent', status: 'idle', createdAt: Date.now() },
                    { id: 'p', name: 'Parent', status: 'idle', createdAt: Date.now(), parentSessionId: 'gp' },
                    { id: 'c', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'p' },
                ]);
            if (cmd === 'agent_delete_session') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(3));

        await act(async () => {
            await result.current.actions.deleteSession('gp');
        });

        // All three nodes must vanish
        await waitFor(() => expect(result.current.state.sessions).toHaveLength(0));
    });

    // ── deleteSessionOnly (orphan) ───────────────────────────────────────────

    it('deleteSessionOnly: removes parent, direct child becomes top-level (parentSessionId cleared)', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'parent', name: 'Parent', status: 'idle', createdAt: Date.now() },
                    { id: 'child', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'parent' },
                ]);
            if (cmd === 'agent_delete_session_only') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        await act(async () => {
            await result.current.actions.deleteSessionOnly('parent');
        });

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        const remaining = result.current.state.sessions[0];
        expect(remaining.id).toBe('child');
        // Direct child must be orphaned (no parentSessionId)
        expect(remaining.parentSessionId).toBeUndefined();
        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session_only', { sessionId: 'parent' });
    });

    it('deleteSessionOnly: grandchild still linked to its own parent after orphan', async () => {
        // Tree: gp → p → c. Delete p with deleteSessionOnly.
        // Expected: gp gone (not deleted here), c.parentSessionId still 'p' (unchanged – p was NOT deleted here)
        // More precisely: only 'p' is removed; 'gp' and 'c' remain; 'c.parentSessionId' stays 'p'
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'gp', name: 'Grandparent', status: 'idle', createdAt: Date.now() },
                    { id: 'p', name: 'Parent', status: 'idle', createdAt: Date.now(), parentSessionId: 'gp' },
                    { id: 'c', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'p' },
                ]);
            if (cmd === 'agent_delete_session_only') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(3));

        await act(async () => {
            // Delete only the middle node – its direct child 'c' is orphaned at top level
            await result.current.actions.deleteSessionOnly('p');
        });

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        const ids = result.current.state.sessions.map((s) => s.id);
        expect(ids).toContain('gp');
        expect(ids).toContain('c');
        expect(ids).not.toContain('p');

        // 'c' was a direct child of 'p' → parentSessionId cleared (now top-level)
        const orphanedChild = result.current.state.sessions.find((s) => s.id === 'c');
        expect(orphanedChild?.parentSessionId).toBeUndefined();

        // 'gp' parentSessionId should be untouched (was already undefined / unrelated)
        const gp = result.current.state.sessions.find((s) => s.id === 'gp');
        expect(gp?.parentSessionId).toBeUndefined();
    });
});

// ─── Regression: toggleBookmark stale-closure fix ───────────────────────────
//
// Before the fix, `newValue` was computed AFTER the optimistic setSessions call
// using the `sessions` closure. React state updates are async, so `sessions`
// still held the pre-flip value — but on rapid double-toggle the second call
// could also see a pre-flip value from the stale closure and send the wrong
// boolean to the backend.
//
// The fix computes `newValue` from the current `sessions` state BEFORE the
// optimistic update so the IPC call always receives the correct intended boolean.

describe('AgentSessionListContext – toggleBookmark', () => {
    const makeSession = (id: string, isBookmarked: boolean) => ({
        id,
        name: `Session ${id}`,
        status: 'idle' as const,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        isBookmarked,
    });

    beforeEach(() => {
        vi.clearAllMocks();
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(vi.fn());
    });

    function setupWithSessions(sessions: ReturnType<typeof makeSession>[]) {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve(sessions);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.resolve(undefined);
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });
    }

    it('toggles bookmark from false → true optimistically', async () => {
        setupWithSessions([makeSession('s1', false)]);

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        expect(result.current.state.sessions[0].isBookmarked).toBe(true);
        expect(safeInvoke).toHaveBeenCalledWith(
            'agent_toggle_session_bookmark',
            expect.objectContaining({ sessionId: 's1', bookmarked: true }),
        );
    });

    it('toggles bookmark from true → false', async () => {
        setupWithSessions([makeSession('s1', true)]);

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        expect(result.current.state.sessions[0].isBookmarked).toBe(false);
        expect(safeInvoke).toHaveBeenCalledWith(
            'agent_toggle_session_bookmark',
            expect.objectContaining({ sessionId: 's1', bookmarked: false }),
        );
    });

    it('reverts optimistic update when IPC call fails', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([makeSession('s1', false)]);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.reject(new Error('network error'));
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1').catch(() => {/* expected */});
        });

        // Should be reverted back to original (false)
        expect(result.current.state.sessions[0].isBookmarked).toBe(false);
    });

    it('sends correct newValue (not stale pre-flip value) to IPC', async () => {
        // This regression test validates that newValue is derived from the current
        // sessions state before the optimistic update, not after.
        setupWithSessions([makeSession('s1', false)]);

        const invokeArgs: unknown[] = [];
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
            invokeArgs.push({ cmd, args });
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([makeSession('s1', false)]);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.resolve(undefined);
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        const toggleCall = (invokeArgs as Array<{ cmd: string; args: { bookmarked: boolean } }>).find(
            (c) => c.cmd === 'agent_toggle_session_bookmark',
        );
        // Must send `true` (the intended new value), not the pre-flip value read
        // from a stale closure after the optimistic update.
        expect(toggleCall?.args.bookmarked).toBe(true);
    });
});
