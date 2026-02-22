import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    AgentSessionListProvider,
    useAgentSessionListState,
    useAgentSessionListActions,
} from '../AgentSessionListContext';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Assistant } from '@/models/chat';

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
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
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

function TestWrapper({ children }: { children: React.ReactNode }) {
    return <AgentSessionListProvider>{children}</AgentSessionListProvider>;
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
        (invoke as ReturnType<typeof vi.fn>).mockResolvedValue([
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

        expect(invoke).toHaveBeenCalledWith('agent_get_all_sessions');
    });

    it('should create a new session', async () => {
        // Mock create response
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
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

        expect(invoke).toHaveBeenCalledWith('agent_create_session', expect.anything());
    });

    it('should delete a session', async () => {
        // Setup: Load one session
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
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

        expect(invoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'session-1' });
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
        (invoke as ReturnType<typeof vi.fn>).mockResolvedValue([
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
        return <AgentSessionListProvider>{children}</AgentSessionListProvider>;
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
        (invoke as ReturnType<typeof vi.fn>).mockResolvedValue([
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

        const invokeCallsBefore = (invoke as ReturnType<typeof vi.fn>).mock.calls.length;

        act(() => {
            agentEventHandler?.({
                payload: { type: 'statusChanged', sessionId: 'session-child', status: 'busy' },
            });
        });

        // Flush microtasks – no new invoke calls expected
        await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
        });

        expect((invoke as ReturnType<typeof vi.fn>).mock.calls.length).toBe(invokeCallsBefore);

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
            expect(agentEventHandler).toBeDefined();
        });

        act(() => {
            agentEventHandler?.({
                payload: { type: 'workflowStarted', sessionId: 'session-child' },
            });
        });

        expect(result.current.sessions[0].status).toBe('paused');
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
        return <AgentSessionListProvider>{children}</AgentSessionListProvider>;
    }

    // ── deleteSession (cascade) ──────────────────────────────────────────────

    it('deleteSession: BFS removes parent AND direct child from UI', async () => {
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
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
        expect(invoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'parent' });
    });

    it('deleteSession: BFS removes entire 3-level tree (grandparent → parent → child)', async () => {
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
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
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
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
        expect(invoke).toHaveBeenCalledWith('agent_delete_session_only', { sessionId: 'parent' });
    });

    it('deleteSessionOnly: grandchild still linked to its own parent after orphan', async () => {
        // Tree: gp → p → c. Delete p with deleteSessionOnly.
        // Expected: gp gone (not deleted here), c.parentSessionId still 'p' (unchanged – p was NOT deleted here)
        // More precisely: only 'p' is removed; 'gp' and 'c' remain; 'c.parentSessionId' stays 'p'
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
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
