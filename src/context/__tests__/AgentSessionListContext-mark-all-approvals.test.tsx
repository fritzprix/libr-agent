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


// ---------------------------------------------------------------------------
// Crash-recovery: statusChanged event patches session list in-place
// ---------------------------------------------------------------------------
describe('AgentSessionListContext – mark-all approval reconciliation', () => {
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

    it('keeps a session that entered the list via backend pending approvals when marking all notifications viewed', async () => {
        // #2971: an approval event for a session not yet in either collection is
        // dropped by the frontend, but the backend records both the attention and
        // the pending count. After the session enters the list on refresh,
        // mark-all-read must keep it because the backend still reports approvals.
        let backendCounts: Record<string, number> = {};

        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_list_sessions') {
                return Promise.resolve([
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
                return Promise.resolve(
                    backendCounts['session-outside']
                        ? [
                              {
                                  id: 'session-outside',
                                  name: 'Outside Session',
                                  status: 'paused',
                                  model: 'gpt-4o',
                                  provider: 'openai',
                                  createdAt: Date.now(),
                                  updatedAt: Date.now(),
                                  lastAttentionAt: Date.now(),
                                  lastAttentionReason: 'pendingApproval',
                              },
                          ]
                        : [],
                );
            }
            if (cmd === 'agent_get_pending_approval_counts') {
                return Promise.resolve({ success: true, message: '', data: backendCounts });
            }
            if (cmd === 'agent_mark_session_viewed') {
                return Promise.resolve();
            }
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
            expect(agentEventHandler).toBeDefined();
        });

        // The outside session is in neither list: the count update is dropped.
        act(() => {
            agentEventHandler?.({
                payload: {
                    type: 'toolExecutionRequiresApproval',
                    sessionId: 'session-outside',
                    toolCallId: 'call-outside',
                },
            });
        });

        expect(result.current.state.notificationSessions.map((s) => s.id)).not.toContain(
            'session-outside',
        );

        // Refresh: backend attention list + backend counts bring the session in
        // with its true pending-approval count.
        backendCounts = { 'session-outside': 2 };
        await act(async () => {
            await result.current.actions.loadSessions(true);
        });

        await waitFor(() => {
            const outside = result.current.state.notificationSessions.find(
                (s) => s.id === 'session-outside',
            );
            expect(outside?.pendingApprovalCount).toBe(2);
        });

        await act(async () => {
            await result.current.actions.markAllNotificationsViewed();
        });

        await waitFor(() => {
            expect(result.current.state.notificationSessions.map((s) => s.id)).toContain(
                'session-outside',
            );
        });
        expect(
            result.current.state.notificationSessions.find((s) => s.id === 'session-outside')
                ?.pendingApprovalCount,
        ).toBe(2);
        expect(safeInvoke).not.toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-outside',
            viewedAt: expect.any(Number),
        });
    });

    it('reconciles a stale zero local approval count with the backend before pruning in mark-all', async () => {
        // #2971 (G1.1a): the bell can hold a session whose local count is stale
        // at 0 because the approval event arrived before the session entered the
        // loaded lists. When the backend still reports approvals at mark-all
        // time, the session must stay with its corrected count.
        let backendCounts: Record<string, number> = {};

        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_list_sessions') {
                return Promise.resolve([
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
                return Promise.resolve([
                    {
                        id: 'session-stale',
                        name: 'Stale Count Session',
                        status: 'paused',
                        model: 'gpt-4o',
                        provider: 'openai',
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                        lastAttentionAt: Date.now(),
                        lastAttentionReason: 'recurringStop',
                    },
                ]);
            }
            if (cmd === 'agent_get_pending_approval_counts') {
                return Promise.resolve({ success: true, message: '', data: backendCounts });
            }
            if (cmd === 'agent_mark_session_viewed') {
                return Promise.resolve();
            }
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
            expect(result.current.state.notificationSessions.map((s) => s.id)).toEqual([
                'session-stale',
            ]);
        });

        // Loaded before any backend count existed: the local count is stale.
        expect(result.current.state.notificationSessions[0]?.pendingApprovalCount).toBe(0);

        // The backend records two approvals for it while the frontend never sees
        // the events (the session was not in the loaded lists when they fired).
        backendCounts = { 'session-stale': 2 };

        await act(async () => {
            await result.current.actions.markAllNotificationsViewed();
        });

        const stale = result.current.state.notificationSessions.find(
            (s) => s.id === 'session-stale',
        );
        expect(stale?.pendingApprovalCount).toBe(2);
        expect(safeInvoke).not.toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-stale',
            viewedAt: expect.any(Number),
        });
    });

    it('prunes a stale notification when the backend no longer reports its local pending approvals', async () => {
        // #2971 (G1.1b): the local count is positive but the backend has no entry
        // for the session, so the approvals are already gone and the stale
        // notification must be marked viewed instead of kept forever.
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
                ]);
            }
            if (cmd === 'agent_list_attention_sessions') {
                return Promise.resolve([]);
            }
            if (cmd === 'agent_get_pending_approval_counts') {
                return Promise.resolve({ success: true, message: '', data: {} });
            }
            if (cmd === 'agent_mark_session_viewed') {
                return Promise.resolve();
            }
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
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
            expect(result.current.state.notificationSessions[0]?.id).toBe('session-child');
            expect(result.current.state.notificationSessions[0]?.pendingApprovalCount).toBe(1);
        });

        await act(async () => {
            await result.current.actions.markAllNotificationsViewed();
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-child',
            viewedAt: expect.any(Number),
        });
        await waitFor(() => {
            expect(result.current.state.notificationSessions).toHaveLength(0);
        });
    });

    it('uses local approval counts in mark-all when the backend count query fails', async () => {
        // #2971 (G1.1 fallback): when the backend query rejects, mark-all must
        // fall back to the frontend counts instead of wiping live counts: the
        // session with a local approval stays, the plain one is marked viewed.
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
            if (cmd === 'agent_get_pending_approval_counts') {
                return Promise.reject(new Error('backend unavailable'));
            }
            if (cmd === 'agent_mark_session_viewed') {
                return Promise.resolve();
            }
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapperWithEvent },
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
        expect(
            result.current.state.notificationSessions[0]?.pendingApprovalCount,
        ).toBe(1);
        expect(safeInvoke).toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-normal',
            viewedAt: expect.any(Number),
        });
        expect(safeInvoke).not.toHaveBeenCalledWith('agent_mark_session_viewed', {
            sessionId: 'session-child',
            viewedAt: expect.any(Number),
        });
    });

});
