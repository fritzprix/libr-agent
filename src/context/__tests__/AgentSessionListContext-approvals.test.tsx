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

vi.mock('@/lib/backend/assistants', () => ({
    getAssistant: vi.fn(),
    listAssistants: vi.fn().mockResolvedValue([]),
}));

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
describe('AgentSessionListContext – approval events', () => {
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

    it('decrements pending approval count only once for a repeated clear of the same tool call', async () => {
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
                    toolCallId: 'call-2',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.state.sessions[0]?.pendingApprovalCount).toBe(2);
        });

        act(() => {
            result.current.actions.clearPendingApproval('session-child', 'call-1');
            result.current.actions.clearPendingApproval('session-child', 'call-1');
            result.current.actions.clearPendingApproval('session-child', 'call-1');
        });

        expect(result.current.state.sessions[0]?.pendingApprovalCount).toBe(1);
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

    it('markAllNotificationsViewed skips sessions with pending approvals', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_list_sessions') {
                return Promise.resolve([
                    {
                        id: 'session-child',
                        name: 'Child Session',
                        status: 'paused',
                        model: 'gpt-4o',
                        provider: 'openai',
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                    },
                    {
                        id: 'session-normal',
                        name: 'Normal Session',
                        status: 'paused',
                        model: 'gpt-4o',
                        provider: 'openai',
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                    },
                ]);
            }
            if (cmd === 'agent_list_attention_sessions') {
                return Promise.resolve([]);
            }
            if (cmd === 'agent_mark_session_viewed') {
                return Promise.resolve();
            }
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({
                state: useAgentSessionListState(),
                actions: useAgentSessionListActions(),
            }),
            {
                wrapper: TestWrapperWithEvent,
            },
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(2);
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
            agentEventHandler?.({
                payload: {
                    type: 'workflowCompleted',
                    sessionId: 'session-normal',
                    reason: 'recurringStop',
                },
            });
        });

        await waitFor(() => {
            expect(result.current.state.notificationSessions).toHaveLength(2);
        });

        await act(async () => {
            await result.current.actions.markAllNotificationsViewed();
        });

        await waitFor(() => {
            expect(result.current.state.notificationSessions.map((s) => s.id)).toEqual([
                'session-child',
            ]);
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-normal',
            viewedAt: expect.any(Number),
        });
        expect(safeInvoke).not.toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-child',
            viewedAt: expect.any(Number),
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

});
