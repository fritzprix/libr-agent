import {
  TEST_SESSION_ID,
  listenMock,
  openAgentSessionMock,
  createOpenSessionResponse,
  setupAgentSessionEventMocks,
} from './agent-session-test-utils';
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { useAgentSessionState } from '../AgentSessionContext';

describe('AgentSessionContext – User prompt events', () => {
  const { mockUnlisten, defaultWrapper } = setupAgentSessionEventMocks();
        it('opens the shared user prompt dialog for a secret request', async () => {
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
                        type: 'userPromptRequested',
                        sessionId: TEST_SESSION_ID,
                        requestId: 'req-1',
                        payload: {
                            kind: 'secret',
                            variableName: 'API_TOKEN',
                            label: 'Token for the deploy',
                        },
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt).toEqual(
                    expect.objectContaining({
                        requestId: 'req-1',
                        kind: 'secret',
                    }),
                );
                expect(result.current.workflowPhase).toBe('waiting_approval');
            });
        });

        it('closes the user prompt dialog on a resolved event for the same request', async () => {
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
                        type: 'userPromptRequested',
                        sessionId: TEST_SESSION_ID,
                        requestId: 'req-1',
                        payload: {
                            kind: 'question',
                            questions: [
                                {
                                    id: 'branch',
                                    question: 'Which branch?',
                                    multiSelect: false,
                                    allowCustom: false,
                                },
                            ],
                        },
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt?.requestId).toBe('req-1');
            });

            // A stale resolve for another prompt must not close this one.
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'userPromptResolved',
                        sessionId: TEST_SESSION_ID,
                        requestId: 'req-other',
                        outcome: 'answered',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt?.requestId).toBe('req-1');
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'userPromptResolved',
                        sessionId: TEST_SESSION_ID,
                        requestId: 'req-1',
                        outcome: 'timeout',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt).toBeNull();
            });
        });

        it('rehydrates a waiting user prompt from the open session payload', async () => {
            openAgentSessionMock.mockResolvedValue(
                createOpenSessionResponse(TEST_SESSION_ID, {
                    pendingUserPrompt: {
                        requestId: 'req-reload',
                        kind: 'secret',
                        payload: {
                            kind: 'secret',
                            variableName: 'API_TOKEN',
                            label: 'Token for the deploy',
                        },
                        createdAt: '2026-11-04T00:00:00.000Z',
                    },
                }),
            );

            const { result } = renderHook(
                () => useAgentSessionState(),
                { wrapper: defaultWrapper }
            );

            await waitFor(() => {
                expect(result.current.isSessionLoading).toBe(false);
                expect(result.current.pendingUserPrompt?.requestId).toBe(
                    'req-reload',
                );
            });
        });

        it('clears a waiting user prompt when the workflow completes', async () => {
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

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'userPromptRequested',
                        sessionId: TEST_SESSION_ID,
                        requestId: 'req-1',
                        payload: {
                            kind: 'secret',
                            variableName: 'API_TOKEN',
                            label: 'Token for the deploy',
                        },
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt).not.toBeNull();
            });

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'workflowCompleted',
                        sessionId: TEST_SESSION_ID,
                        reason: 'terminated',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.pendingUserPrompt).toBeNull();
            });
        });
});
