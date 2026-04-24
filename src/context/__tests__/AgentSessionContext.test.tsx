import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    AgentSessionProvider,
    useAgentSessionActions,
    useAgentSessionState,
} from '../AgentSessionContext';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import * as messagesBackend from '@/lib/backend/messages';
import type { Message } from '@/models/chat';

const mockMarkSessionViewed = vi.fn();
const mockClearPendingApproval = vi.fn();
const mockRefreshCompactedRange = vi.fn();

// Mock Tauri APIs
vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

vi.mock('@/lib/backend/core', () => ({
    safeInvoke: vi.fn(),
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

// Mock backend messages API
vi.mock('@/lib/backend/messages', () => ({
    getMessagesPageForSession: vi.fn(),
}));

vi.mock('../AgentSessionListContext', () => ({
    useAgentSessionListActions: () => ({
        markSessionViewed: mockMarkSessionViewed,
        clearPendingApproval: mockClearPendingApproval,
    }),
}));

vi.mock('../LLMServiceContext', () => ({
    useLLMService: () => ({
        refreshCompactedRange: mockRefreshCompactedRange,
    }),
}));

// Mock ModelProvider
vi.mock('../ModelProvider', () => ({
    useModelOptions: () => ({
        modelId: 'test-model',
        provider: 'test-provider',
    }),
}));

const TEST_SESSION_ID = 'session-1';

function TestWrapper({ children }: { children: React.ReactNode }) {
    // Provide a mocked sessionId prop
    return <AgentSessionProvider sessionId={TEST_SESSION_ID}>{children}</AgentSessionProvider>;
}

function DynamicSessionWrapper({
    children,
    sessionId,
}: {
    children: React.ReactNode;
    sessionId: string;
}) {
    return <AgentSessionProvider sessionId={sessionId}>{children}</AgentSessionProvider>;
}

describe('AgentSessionContext (Local)', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        mockMarkSessionViewed.mockResolvedValue(undefined);
        mockRefreshCompactedRange.mockResolvedValue(undefined);
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
        // Mock getMessagesPageForSession to return empty list by default
        (messagesBackend.getMessagesPageForSession as ReturnType<typeof vi.fn>).mockResolvedValue({
            items: [],
            total: 0,
            page: 1,
            pageSize: 50,
            totalPages: 0,
        });

        // Mock get_session interaction for initialization
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue({
            id: TEST_SESSION_ID,
            name: 'Test Session',
            status: 'idle',
            createdAt: Date.now(),
            updatedAt: Date.now(),
            yoloMode: false,
        });
    });

    it('should initialize with session state', async () => {
        const { result } = renderHook(() => useAgentSessionState(), {
            wrapper: TestWrapper,
        });

        // Initially loading
        expect(result.current.isSessionLoading).toBe(true);

        // Wait for load
        await waitFor(() => {
            expect(result.current.isSessionLoading).toBe(false);
        });

        expect(result.current.session?.id).toBe(TEST_SESSION_ID);
        expect(result.current.workflowStatus).toBe('idle');
        expect(result.current.messages).toEqual([]);

        expect(safeInvoke).toHaveBeenCalledWith('agent_get_session', { sessionId: TEST_SESSION_ID });
        expect(mockRefreshCompactedRange).toHaveBeenCalledWith(TEST_SESSION_ID);
    });

    it('should register event listener for the session', async () => {
        renderHook(() => useAgentSessionState(), {
            wrapper: TestWrapper,
        });

        await waitFor(() => {
            expect(listen).toHaveBeenCalledWith('agent:event', expect.any(Function));
        });
    });

    it('does not reinitialize the session when rerendered with the same sessionId', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((command, args) => {
            if (command === 'agent_get_session') {
                return Promise.resolve({
                    id: args.sessionId,
                    name: `Session ${args.sessionId}`,
                    status: 'idle',
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                    yoloMode: false,
                });
            }

            if (command === 'messages_get_page') {
                return Promise.resolve({
                    items: [],
                    total: 0,
                    page: 1,
                    pageSize: 1000,
                    totalPages: 0,
                });
            }

            if (
                command === 'agent_resume_session' ||
                command === 'agent_init_session_with_messages'
            ) {
                return Promise.resolve({});
            }

            return Promise.resolve(undefined);
        });

        const { result, rerender } = renderHook(() => useAgentSessionState(), {
            wrapper: ({ children }) => (
                <DynamicSessionWrapper sessionId="session-1">
                    {children}
                </DynamicSessionWrapper>
            ),
        });

        await waitFor(() => {
            expect(result.current.isSessionLoading).toBe(false);
            expect(result.current.session?.id).toBe('session-1');
        });

        expect(mockRefreshCompactedRange).toHaveBeenCalledTimes(1);
        expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);

        const initialGetSessionCalls = (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
            ([command]) => command === 'agent_get_session'
        ).length;
        const initialMessagePageCalls = (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
            ([command]) => command === 'messages_get_page'
        ).length;

        rerender();

        expect(
            (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                ([command]) => command === 'agent_get_session'
            )
        ).toHaveLength(initialGetSessionCalls);
        expect(
            (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                ([command]) => command === 'messages_get_page'
            )
        ).toHaveLength(initialMessagePageCalls);
        expect(mockRefreshCompactedRange).toHaveBeenCalledTimes(1);
        expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
    });

    it('reinitializes exactly once when the active sessionId changes', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((command, args) => {
            if (command === 'agent_get_session') {
                return Promise.resolve({
                    id: args.sessionId,
                    name: `Session ${args.sessionId}`,
                    status: 'idle',
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                    yoloMode: false,
                });
            }

            if (command === 'messages_get_page') {
                return Promise.resolve({
                    items: [],
                    total: 0,
                    page: 1,
                    pageSize: 1000,
                    totalPages: 0,
                });
            }

            if (
                command === 'agent_resume_session' ||
                command === 'agent_init_session_with_messages'
            ) {
                return Promise.resolve({});
            }

            return Promise.resolve(undefined);
        });

        let activeSessionId = 'session-1';
        const Wrapper = ({ children }: { children: React.ReactNode }) => (
            <DynamicSessionWrapper sessionId={activeSessionId}>
                {children}
            </DynamicSessionWrapper>
        );

        const { result, rerender } = renderHook(() => useAgentSessionState(), {
            wrapper: Wrapper,
        });

        await waitFor(() => {
            expect(result.current.isSessionLoading).toBe(false);
            expect(result.current.session?.id).toBe('session-1');
        });

        activeSessionId = 'session-2';
        rerender();

        await waitFor(() => {
            expect(result.current.isSessionLoading).toBe(false);
            expect(result.current.session?.id).toBe('session-2');
        });

        expect(mockRefreshCompactedRange).toHaveBeenNthCalledWith(1, 'session-1');
        expect(mockRefreshCompactedRange).toHaveBeenNthCalledWith(2, 'session-2');

        expect(
            (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                ([command, args]) =>
                    command === 'agent_get_session' && args.sessionId === 'session-2'
            )
        ).toHaveLength(1);
        expect(
            (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                ([command, args]) =>
                    command === 'messages_get_page' && args.sessionId === 'session-2'
            )
        ).toHaveLength(1);
        expect(mockMarkSessionViewed).toHaveBeenNthCalledWith(
            2,
            'session-2',
            expect.any(Date)
        );
    });

    describe('Event Handling', () => {
        it('should update workflow status on statusChanged event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(result.current.isSessionLoading).toBe(false);
                expect(eventHandler).toBeDefined();
            });

            // Emit statusChanged event
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'statusChanged',
                        sessionId: TEST_SESSION_ID,
                        status: 'busy',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.workflowStatus).toBe('busy');
            });
        });

        it('keeps cancelled workflows paused when workflowCompleted is emitted', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(result.current.isSessionLoading).toBe(false);
                expect(eventHandler).toBeDefined();
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'workflowCompleted',
                        sessionId: TEST_SESSION_ID,
                        reason: 'cancelled',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.workflowStatus).toBe('paused');
                expect(result.current.workflowPhase).toBe('idle');
            });
        });

        it('should append message on messageAdded event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            const newMessage: Message = {
                id: 'msg-1',
                sessionId: TEST_SESSION_ID,
                threadId: TEST_SESSION_ID,
                role: 'user',
                content: [{ type: 'text', text: 'Hello' }],
                createdAt: new Date(),
            } as unknown as Message;

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'messageAdded',
                        sessionId: TEST_SESSION_ID,
                        message: newMessage,
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.messages).toHaveLength(1);
                expect(result.current.messages[0].id).toBe('msg-1');
            });
        });

        it('should ignore events from other sessions', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit statusChanged for DIFFERENT session
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'statusChanged',
                        sessionId: 'OTHER-SESSION',
                        status: 'busy',
                    },
                });
            });

            // Status should remain idle (default mock status)
            expect(result.current.workflowStatus).toBe('idle');
        });

        it('should handle workflowError event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit workflowError event
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'workflowError',
                        sessionId: TEST_SESSION_ID,
                        error: {
                            type: 'AI_SERVICE_ERROR',
                            displayMessage: 'Something went wrong',
                            recoverable: true,
                            details: {
                                originalError: 'Something went wrong',
                                timestamp: '2026-03-14T00:00:00.000Z',
                            },
                        },
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.workflowStatus).toBe('error');
                expect(result.current.error).toEqual(
                    expect.objectContaining({
                        type: 'AI_SERVICE_ERROR',
                        displayMessage: 'Something went wrong',
                    }),
                );
            });
        });
    });

    describe('Notification acknowledgement', () => {
        it('marks the session viewed after resuming the workflow', async () => {
            const { result } = renderHook(
                () => useAgentSessionActions(),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
            });

            mockMarkSessionViewed.mockClear();

            await act(async () => {
                await result.current.resumeSession();
            });

            expect(safeInvoke).toHaveBeenCalledWith('agent_resume_workflow', {
                sessionId: TEST_SESSION_ID,
            });
            expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
            expect(mockMarkSessionViewed).toHaveBeenCalledWith(
                TEST_SESSION_ID,
                expect.any(Date),
            );
        });

        it('clears global approval counts and marks the session viewed when YOLO auto-approves pending tools', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => ({
                    state: useAgentSessionState(),
                    actions: useAgentSessionActions(),
                }),
                { wrapper: TestWrapper }
            );

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
                expect(result.current.state.isSessionLoading).toBe(false);
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'toolExecutionRequiresApproval',
                        sessionId: TEST_SESSION_ID,
                        toolCallId: 'call-1',
                        toolName: 'shell',
                        arguments: '{}',
                    },
                });
                eventHandler?.({
                    payload: {
                        type: 'toolExecutionRequiresApproval',
                        sessionId: TEST_SESSION_ID,
                        toolCallId: 'call-2',
                        toolName: 'shell',
                        arguments: '{}',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.state.pendingApprovals).toHaveLength(2);
            });

            mockMarkSessionViewed.mockClear();
            mockClearPendingApproval.mockClear();

            await act(async () => {
                await result.current.actions.toggleYoloMode();
            });

            expect(safeInvoke).toHaveBeenCalledWith('agent_set_yolo_mode', {
                sessionId: TEST_SESSION_ID,
                enabled: true,
            });
            expect(safeInvoke).toHaveBeenCalledWith('agent_respond_tool_approval', {
                sessionId: TEST_SESSION_ID,
                toolCallId: 'call-1',
                approved: true,
            });
            expect(safeInvoke).toHaveBeenCalledWith('agent_respond_tool_approval', {
                sessionId: TEST_SESSION_ID,
                toolCallId: 'call-2',
                approved: true,
            });
            expect(mockClearPendingApproval).toHaveBeenCalledTimes(2);
            expect(mockClearPendingApproval).toHaveBeenNthCalledWith(
                1,
                TEST_SESSION_ID,
                'call-1',
            );
            expect(mockClearPendingApproval).toHaveBeenNthCalledWith(
                2,
                TEST_SESSION_ID,
                'call-2',
            );
            expect(mockMarkSessionViewed).toHaveBeenCalledWith(
                TEST_SESSION_ID,
                expect.any(Date),
            );
        });
    });
});
