import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    __resetAgentSessionListStartupCacheForTests,
    AgentSessionListProvider,
    useAgentSessionListState,
    useAgentSessionListActions,
} from '../AgentSessionListContext';
import { safeInvoke } from '@/lib/backend/core';
import { listen } from '@tauri-apps/api/event';
import { MemoryRouter } from 'react-router-dom';

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

function TestWrapperWithEvent({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

function ActiveSessionWrapper({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter initialEntries={['/agent/session-child']}>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

// ---------------------------------------------------------------------------
// Crash-recovery: statusChanged event patches session list in-place
// ---------------------------------------------------------------------------
describe('AgentSessionListContext – statusChanged event (crash recovery)', () => {
    const mockUnlisten = vi.fn();

    // Capture the agent:event handler registered by the useEffect listen() call
    let agentEventHandler: ((event: { payload: unknown }) => void) | undefined;

    beforeEach(() => {
        vi.clearAllMocks();
        __resetAgentSessionListStartupCacheForTests();
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

        expect(safeInvoke).toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-child',
            viewedAt: expect.any(Number),
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

    it('tracks approval requests as unread notifications for inactive sessions until viewed', async () => {
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
        });

        await waitFor(() => {
            expect(result.current.state.unreadNotificationCount).toBe(1);
            expect(result.current.state.notificationSessions[0]?.id).toBe('session-child');
            expect(result.current.state.notificationSessions[0]?.lastAttentionReason).toBe('pendingApproval');
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
            expect(result.current.state.sessions[0]?.pendingApprovalCount).toBe(1);
        });
    });

    it('preserves the latest pending approval count when another attention event follows immediately', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapperWithEvent,
        });

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
                    type: 'workflowCompleted',
                    sessionId: 'session-child',
                    reason: 'recurringStop',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.notificationSessions[0]?.id).toBe('session-child');
            expect(result.current.notificationSessions[0]?.pendingApprovalCount).toBe(1);
            expect(result.current.notificationSessions[0]?.lastAttentionReason).toBe('recurringStop');
        });
    });

    it('marks active-session approval requests as viewed immediately', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: ActiveSessionWrapper,
        });

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
        });

        await waitFor(() => {
            expect(result.current.sessions[0]?.pendingApprovalCount).toBe(1);
            expect(result.current.sessions[0]?.lastAttentionReason).toBeUndefined();
            expect(result.current.unreadNotificationCount).toBe(0);
            expect(result.current.notificationSessions).toHaveLength(0);
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-child',
            viewedAt: expect.any(Number),
        });
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
