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
describe('AgentSessionListContext – notification events', () => {
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

});
