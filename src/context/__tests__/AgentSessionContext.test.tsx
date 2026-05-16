import { TEST_SESSION_ID, SessionWrapper, mockMarkSessionViewed, mockRefreshCompactedRange, listenMock, openAgentSessionMock, safeInvokeMock, createOpenSessionResponse, AgentSessionStateObserver, AgentSessionStateSnapshot, OpenAgentSessionResponse } from "./agent-session-test-utils";
import { render, renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    useAgentSessionState,
} from '../AgentSessionContext';
import { listen } from '@tauri-apps/api/event';
describe('AgentSessionContext (Local)', () => {
  const mockUnlisten = vi.fn();
  const defaultWrapper = ({ children }: React.PropsWithChildren) => (
    <SessionWrapper sessionId={TEST_SESSION_ID}>{children}</SessionWrapper>
  );

  beforeEach(() => {
    vi.clearAllMocks();
    mockMarkSessionViewed.mockResolvedValue(undefined);
    mockRefreshCompactedRange.mockResolvedValue(undefined);
    listenMock.mockResolvedValue(mockUnlisten);
    openAgentSessionMock.mockResolvedValue(
      createOpenSessionResponse(TEST_SESSION_ID, {
        session: {
          name: 'Test Session',
        },
      }),
    );
    safeInvokeMock.mockResolvedValue(undefined);
  });

    it('should initialize with session state', async () => {
        const { result } = renderHook(() => useAgentSessionState(), {
            wrapper: defaultWrapper,
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

        expect(openAgentSessionMock).toHaveBeenCalledWith(TEST_SESSION_ID);
        expect(mockRefreshCompactedRange).toHaveBeenCalledWith(TEST_SESSION_ID);
    });

    it('should register event listener for the session', async () => {
        renderHook(() => useAgentSessionState(), {
            wrapper: defaultWrapper,
        });

        await waitFor(() => {
            expect(listen).toHaveBeenCalledWith('agent:event', expect.any(Function));
        });
    });

    it('does not reinitialize the session when rerendered with the same sessionId', async () => {
        openAgentSessionMock.mockImplementation(async (sessionId: string) =>
            createOpenSessionResponse(sessionId)
        );

        let latestState: AgentSessionStateSnapshot | undefined;
        const currentState = () => {
            if (!latestState) {
                throw new Error('Expected session state snapshot');
            }
            return latestState;
        };
        const { rerender } = render(
            <SessionWrapper sessionId="session-1">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-1');
        });

        expect(mockRefreshCompactedRange).toHaveBeenCalledTimes(1);
        expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);

        const initialOpenSessionCalls = openAgentSessionMock.mock.calls.length;

        rerender(
            <SessionWrapper sessionId="session-1">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        expect(openAgentSessionMock).toHaveBeenCalledTimes(
            initialOpenSessionCalls
        );
        expect(mockRefreshCompactedRange).toHaveBeenCalledTimes(1);
        expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
    });

    it('reinitializes exactly once when the active sessionId changes', async () => {
        openAgentSessionMock.mockImplementation(async (sessionId: string) =>
            createOpenSessionResponse(sessionId)
        );

        let latestState: AgentSessionStateSnapshot | undefined;
        const currentState = () => {
            if (!latestState) {
                throw new Error('Expected session state snapshot');
            }
            return latestState;
        };
        const { rerender } = render(
            <SessionWrapper sessionId="session-1">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-1');
        });

        rerender(
            <SessionWrapper sessionId="session-2">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-2');
        });

        expect(mockRefreshCompactedRange).toHaveBeenNthCalledWith(1, 'session-1');
        expect(mockRefreshCompactedRange).toHaveBeenNthCalledWith(2, 'session-2');

        expect(openAgentSessionMock).toHaveBeenCalledWith('session-2');
        expect(mockMarkSessionViewed).toHaveBeenNthCalledWith(
            2,
            'session-2',
            expect.any(Date)
        );
    });

    it('keeps the previous session state visible while the next session hydrates', async () => {
        let resolveNextSession:
            | ((
                  value: OpenAgentSessionResponse,
              ) => void)
            | undefined;

        openAgentSessionMock.mockImplementation((sessionId: string) => {
                if (sessionId === 'session-2') {
                    return new Promise((resolve) => {
                        resolveNextSession = resolve;
                    });
                }

                return Promise.resolve(createOpenSessionResponse(sessionId));
            }
        );

        let latestState: AgentSessionStateSnapshot | undefined;
        const currentState = () => {
            if (!latestState) {
                throw new Error('Expected session state snapshot');
            }
            return latestState;
        };
        const { rerender } = render(
            <SessionWrapper sessionId="session-1">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-1');
        });

        rerender(
            <SessionWrapper sessionId="session-2">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(true);
        });

        expect(currentState().session?.id).toBe('session-1');

        await waitFor(() => {
            expect(resolveNextSession).toBeDefined();
        });

        await act(async () => {
            resolveNextSession?.(createOpenSessionResponse('session-2'));
        });

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-2');
        });
    });

    it('clears stale session state when the next session fails to hydrate', async () => {
        let rejectNextSession: ((reason?: unknown) => void) | undefined;

        openAgentSessionMock.mockImplementation((sessionId: string) => {
                if (sessionId === 'session-2') {
                    return new Promise((_, reject) => {
                        rejectNextSession = reject;
                    });
                }

                return Promise.resolve(
                    createOpenSessionResponse(sessionId, {
                        session: {
                            yoloMode: true,
                        },
                messages: [
                    {
                        id: 'message-1',
                        sessionId,
                        threadId: sessionId,
                        role: 'assistant',
                        content: [{ type: 'text', text: 'Existing message' }],
                        createdAt: Date.now(),
                        updatedAt: Date.now(),
                    },
                ],
                        pendingApprovals: [
                            {
                                toolCallId: 'tool-1',
                                toolName: 'write_file',
                                arguments: '{}',
                                approvalKind: 'standard',
                                requestId: 'req-1',
                                description: 'Approve write_file',
                                inputPreview: '{}',
                            },
                        ],
                    })
                );
            }
        );

        let latestState: AgentSessionStateSnapshot | undefined;
        const currentState = () => {
            if (!latestState) {
                throw new Error('Expected session state snapshot');
            }
            return latestState;
        };
        const { rerender } = render(
            <SessionWrapper sessionId="session-1">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session?.id).toBe('session-1');
            expect(currentState().messages).toHaveLength(1);
            expect(currentState().pendingApprovals).toHaveLength(1);
        });
        expect(currentState().pendingApprovals[0]).toMatchObject({
            toolCallId: 'tool-1',
            approvalKind: 'standard',
            requestId: 'req-1',
            description: 'Approve write_file',
            inputPreview: '{}',
        });

        rerender(
            <SessionWrapper sessionId="session-2">
                <AgentSessionStateObserver
                    onRender={(state) => {
                        latestState = state;
                    }}
                />
            </SessionWrapper>
        );

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(true);
        });

        expect(currentState().session?.id).toBe('session-1');

        await act(async () => {
            rejectNextSession?.(new Error('Session open failed'));
        });

        await waitFor(() => {
            expect(currentState().isSessionLoading).toBe(false);
            expect(currentState().session).toBeNull();
        });

        expect(currentState().messages).toEqual([]);
        expect(currentState().pendingApprovals).toEqual([]);
        expect(currentState().workflowStatus).toBe('error');
        expect(currentState().error?.displayMessage).toContain('Session open failed');
    });

});
