import { TEST_SESSION_ID, createDefaultWrapper, mockMarkSessionViewed, mockRefreshCompactedRange, listenMock, openAgentSessionMock, safeInvokeMock, createOpenSessionResponse, mockClearPendingApproval } from "./agent-session-test-utils";
import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    useAgentSessionActions,
    useAgentSessionState,
} from '../AgentSessionContext';
describe('AgentSessionContext – User Actions', () => {
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

        it('marks the session viewed after resuming the workflow', async () => {
            const { result } = renderHook(
                () => useAgentSessionActions(),
                { wrapper: defaultWrapper }
            );

            await waitFor(() => {
                expect(mockMarkSessionViewed).toHaveBeenCalledTimes(1);
            });

            mockMarkSessionViewed.mockClear();

            await act(async () => {
                await result.current.resumeSession();
            });

            expect(safeInvokeMock).toHaveBeenCalledWith('agent_resume_workflow', {
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
            listenMock.mockImplementation(
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
                { wrapper: defaultWrapper }
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
                        approvalKind: 'standard',
                        requestId: 'req-1',
                        description: 'Approve shell',
                        inputPreview: '{}',
                    },
                });
                eventHandler?.({
                    payload: {
                        type: 'toolExecutionRequiresApproval',
                        sessionId: TEST_SESSION_ID,
                        toolCallId: 'call-2',
                        toolName: 'shell',
                        arguments: '{}',
                        approvalKind: 'hard',
                        requestId: 'req-2',
                        description: 'Approve shell',
                        inputPreview: '{}',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.state.pendingApprovals).toHaveLength(2);
            });
            expect(result.current.state.pendingApprovals[0]).toMatchObject({
                toolCallId: 'call-1',
                approvalKind: 'standard',
                requestId: 'req-1',
                description: 'Approve shell',
                inputPreview: '{}',
            });

            mockMarkSessionViewed.mockClear();
            mockClearPendingApproval.mockClear();

            await act(async () => {
                await result.current.actions.toggleYoloMode();
            });

            expect(safeInvokeMock).toHaveBeenCalledWith('agent_set_execution_mode', {
                sessionId: TEST_SESSION_ID,
                mode: 'yolo',
            });
            expect(safeInvokeMock).not.toHaveBeenCalledWith('agent_respond_tool_approval', {
                sessionId: TEST_SESSION_ID,
                toolCallId: 'call-1',
                approved: true,
            });
            expect(safeInvokeMock).not.toHaveBeenCalledWith('agent_respond_tool_approval', {
                sessionId: TEST_SESSION_ID,
                toolCallId: 'call-2',
                approved: true,
            });
            expect(mockClearPendingApproval).not.toHaveBeenCalled();
            expect(mockMarkSessionViewed).toHaveBeenCalledWith(
                TEST_SESSION_ID,
                expect.any(Date),
            );
        });

        it('switches execution mode exclusively when unsafe mode is selected', async () => {
            const { result } = renderHook(
                () => ({
                    state: useAgentSessionState(),
                    actions: useAgentSessionActions(),
                }),
                { wrapper: defaultWrapper }
            );

            await waitFor(() => {
                expect(result.current.state.isSessionLoading).toBe(false);
            });

            safeInvokeMock.mockClear();

            await act(async () => {
                await result.current.actions.setExecutionMode('unsafe');
            });

            expect(safeInvokeMock).toHaveBeenCalledTimes(1);
            expect(safeInvokeMock).toHaveBeenNthCalledWith(1, 'agent_set_execution_mode', {
                sessionId: TEST_SESSION_ID,
                mode: 'unsafe',
            });
            expect(result.current.state.executionMode).toBe('unsafe');
            expect(result.current.state.yoloModeEnabled).toBe(false);
            expect(result.current.state.unsafeModeEnabled).toBe(true);
        });
});
