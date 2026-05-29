import { TEST_SESSION_ID, createDefaultWrapper, mockMarkSessionViewed, mockRefreshCompactedRange, listenMock, openAgentSessionMock, safeInvokeMock, createOpenSessionResponse, createReadyRuntimeState, buildTestMessage, OpenAgentSessionResponse } from "./agent-session-test-utils";
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    useAgentSessionState,
} from '../AgentSessionContext';
describe('AgentSessionContext – Event Handling', () => {
  const mockUnlisten = vi.fn();
  const defaultWrapper = createDefaultWrapper();

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

        it('ignores stale runtime-state snapshots that arrive after a newer one', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            let resolveOpenSession:
                | ((
                      value: OpenAgentSessionResponse,
                  ) => void)
                | undefined;

            listenMock.mockImplementation(
                async (_eventName, handler) => {
                    eventHandler = handler;
                    return mockUnlisten;
                },
            );
            openAgentSessionMock.mockImplementation(
                    () =>
                        new Promise((resolve) => {
                            resolveOpenSession = resolve;
                        }),
                );

            const { result } = renderHook(() => useAgentSessionState(), {
                wrapper: defaultWrapper,
            });

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'sessionRuntimeStateUpdated',
                        sessionId: TEST_SESSION_ID,
                        runtimeState: createReadyRuntimeState({
                            sequence: 3,
                            initialization: {
                                currentStep: 'Newer runtime state',
                                result: 'success' as const,
                            },
                        }),
                    },
                });
            });

            await act(async () => {
                resolveOpenSession?.(
                    createOpenSessionResponse(TEST_SESSION_ID, {
                        session: {
                            name: 'Test Session',
                        },
                    }),
                );
            });

            await waitFor(() => {
                expect(result.current.runtimeState.sequence).toBe(3);
                expect(result.current.runtimeState.initialization.currentStep).toBe(
                    'Newer runtime state',
                );
            });
        });

        it('ignores stale runtime-state events that arrive after a newer event snapshot', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            listenMock.mockImplementation(
                async (_eventName, handler) => {
                    eventHandler = handler;
                    return mockUnlisten;
                },
            );

            const { result } = renderHook(() => useAgentSessionState(), {
                wrapper: defaultWrapper,
            });

            await waitFor(() => {
                expect(result.current.isSessionLoading).toBe(false);
                expect(eventHandler).toBeDefined();
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'sessionRuntimeStateUpdated',
                        sessionId: TEST_SESSION_ID,
                        runtimeState: createReadyRuntimeState({
                            sequence: 4,
                            initialization: {
                                currentStep: 'Newest runtime state',
                                result: 'success' as const,
                            },
                        }),
                    },
                });
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'sessionRuntimeStateUpdated',
                        sessionId: TEST_SESSION_ID,
                        runtimeState: createReadyRuntimeState({
                            sequence: 2,
                            initialization: {
                                currentStep: 'Stale runtime state',
                                result: 'success' as const,
                            },
                        }),
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.runtimeState.sequence).toBe(4);
                expect(result.current.runtimeState.initialization.currentStep).toBe(
                    'Newest runtime state',
                );
            });
        });

        it('should update workflow status on statusChanged event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
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

        it('stores backend preflight token metrics from agent events', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
            );

            await waitFor(() => {
                expect(result.current.isSessionLoading).toBe(false);
                expect(eventHandler).toBeDefined();
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'preflightTokenMetricsUpdated',
                        sessionId: TEST_SESSION_ID,
                        metrics: {
                            conservativePromptTokens: 2450,
                            promptAnchoredTotalTokens: 2334,
                            safeInputTokenLimit: 65536,
                            measuredOutputTokensReserve: 1200,
                            effectiveInputBudget: 64336,
                            totalBudgetTokens: 3650,
                            systemPromptTokens: 2048,
                            toolsTokens: 1024,
                            selectedMessageCount: 12,
                            compactSummaryInjected: true,
                            preservedCalibrationRatio: 0.92,
                        },
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.preflightTokenMetrics).toEqual({
                    conservativePromptTokens: 2450,
                    promptAnchoredTotalTokens: 2334,
                    safeInputTokenLimit: 65536,
                    measuredOutputTokensReserve: 1200,
                    effectiveInputBudget: 64336,
                    totalBudgetTokens: 3650,
                    systemPromptTokens: 2048,
                    toolsTokens: 1024,
                    selectedMessageCount: 12,
                    compactSummaryInjected: true,
                    preservedCalibrationRatio: 0.92,
                });
            });
        });

        it('keeps cancelled workflows paused when workflowCompleted is emitted', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
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
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
            );

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            const newMessage = buildTestMessage();

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
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
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
            listenMock.mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
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
