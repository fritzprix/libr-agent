import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import { openAgentSession } from '@/lib/backend/agent-commands';
import { getLogger } from '@/lib/logger';
import { rustMessageToMessage } from '@/models/chat';
import type { AgentSession } from '@/models/agent';
import type { AgentResponse } from '@/models/agent-ipc';
import type { AgentEventPayload } from './types';
import { buildMessageError } from './utils';
import type { useAgentSessionState } from './useAgentSessionState';
import type { SessionRuntimeState } from '@/models/agent-ipc';

const logger = getLogger('AgentSessionEvents');

const HYDRATING_RUNTIME_STATE: SessionRuntimeState = {
  sequence: 0,
  phase: 'hydrating',
  proxy: {
    exists: false,
    mode: 'none',
    ready: false,
  },
  initialization: {
    currentStep: 'Starting session...',
    result: 'pending',
  },
  servers: [],
};

function createRuntimeFailureState(errorMessage: string): SessionRuntimeState {
  return {
    sequence: 0,
    phase: 'failed',
    proxy: {
      exists: false,
      mode: 'none',
      ready: false,
    },
    initialization: {
      currentStep: 'Failed to open session',
      result: 'failed',
      error: errorMessage,
    },
    servers: [],
  };
}

function shouldApplyRuntimeState(
  currentState: SessionRuntimeState,
  nextState: SessionRuntimeState,
): boolean {
  return nextState.sequence >= currentState.sequence;
}

export function useAgentSessionEvents(
  sessionId: string,
  stateProps: ReturnType<typeof useAgentSessionState>,
  actions: {
    persistViewedAt: (viewedAt?: Date) => Promise<void>;
  },
) {
  const { setters, refs } = stateProps;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isMounted = true;

    const initSession = async () => {
      logger.info('Initializing agent session', { sessionId });
      setters.setError(null);
      setters.setRuntimeState(HYDRATING_RUNTIME_STATE);

      try {
        unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
          if (!isMounted) return;

          const payload = event.payload;

          if (
            payload.type !== 'resourceUpdated' &&
            'sessionId' in payload &&
            payload.sessionId !== sessionId
          ) {
            return;
          }

          switch (payload.type) {
            case 'sessionRuntimeStateUpdated': {
              setters.setRuntimeState((currentState) =>
                shouldApplyRuntimeState(currentState, payload.runtimeState)
                  ? payload.runtimeState
                  : currentState,
              );
              break;
            }

            case 'workflowStarted': {
              setters.setWorkflowStatus('busy');
              setters.setWorkflowPhase('thinking');
              setters.setError(null);
              setters.setLlmError(null);
              logger.info('Workflow phase: thinking');
              break;
            }

            case 'statusChanged': {
              const newStatus = payload.status;
              setters.setWorkflowStatus(newStatus);
              setters.setSession((prev) =>
                prev ? { ...prev, status: newStatus } : null,
              );

              if (newStatus === 'busy') {
                setters.setError(null);
                setters.setLlmError(null);
                setters.setWorkflowPhase('thinking');
              } else if (newStatus === 'idle') {
                setters.setWorkflowPhase('idle');
              } else if (newStatus === 'error') {
                setters.setWorkflowPhase('error');
              }
              break;
            }

            case 'workflowError': {
              setters.setWorkflowStatus('error');
              const nextError = buildMessageError(payload.error);

              if (
                nextError.type === 'MALFORMED_FUNCTION_CALL' ||
                nextError.type === 'JSON_PARSING_ERROR' ||
                nextError.type === 'EMPTY_SELECTION_ERROR'
              ) {
                setters.setLlmError(nextError);
                setters.setError(null);
              } else {
                setters.setError(nextError);
                setters.setLlmError(null);
              }
              break;
            }

            case 'messageAdded': {
              const rustMessage = payload.message;
              const newMessage = rustMessageToMessage(rustMessage);

              if (
                newMessage.role === 'assistant' &&
                newMessage.isStreaming &&
                refs.workflowPhaseRef.current === 'thinking'
              ) {
                setters.setWorkflowPhase('answering');
                logger.info('Workflow phase: answering');
              }

              setters.setMessages((prev) => {
                if (prev.some((m) => m.id === newMessage.id)) return prev;
                return [...prev, newMessage];
              });

              if (!newMessage.isStreaming) {
                setters.applyLocalViewedAt(new Date(rustMessage.createdAt));
              }

              if (
                newMessage.role === 'assistant' &&
                !newMessage.isStreaming &&
                newMessage.thinking &&
                (!newMessage.content || newMessage.content.length === 0) &&
                (!newMessage.tool_calls || newMessage.tool_calls.length === 0)
              ) {
                logger.info(
                  'Detected Think-Only message, triggering recurring request',
                  {
                    messageId: newMessage.id,
                  },
                );

                setTimeout(() => {
                  safeInvoke<AgentResponse>('agent_resume_workflow', {
                    sessionId,
                  }).catch((err) => {
                    logger.error(
                      'Failed to trigger recurring request for thinking message',
                      err,
                    );
                  });
                }, 100);
              }

              break;
            }

            case 'toolExecutionStarted': {
              setters.setWorkflowPhase('using_tools');
              logger.info('Workflow phase: using_tools', {
                toolName: payload.toolName,
              });
              break;
            }

            case 'toolExecutionRequiresApproval': {
              if (refs.yoloModeRef.current) {
                logger.info(
                  'YOLO Mode enabled: Auto-approving tool execution',
                  {
                    toolName: payload.toolName,
                    toolCallId: payload.toolCallId,
                  },
                );

                safeInvoke<AgentResponse>('agent_respond_tool_approval', {
                  sessionId,
                  toolCallId: payload.toolCallId,
                  approved: true,
                }).catch((err) => {
                  logger.error('Failed to auto-approve tool in YOLO mode', err);
                });

                setters.setWorkflowPhase('using_tools');
                break;
              }

              setters.setWorkflowPhase('waiting_approval');
              setters.setPendingApprovals((prev) => {
                if (prev.some((p) => p.toolCallId === payload.toolCallId)) {
                  return prev;
                }
                return [
                  ...prev,
                  {
                    toolCallId: payload.toolCallId,
                    toolName: payload.toolName,
                    arguments: payload.arguments,
                  },
                ];
              });
              logger.info('Workflow phase: waiting_approval', {
                toolName: payload.toolName,
              });
              break;
            }

            case 'toolExecutionCompleted': {
              break;
            }

            case 'toolExecutionApprovalResolved': {
              setters.setPendingApprovals((prev) =>
                prev.filter((p) => p.toolCallId !== payload.toolCallId),
              );

              if (payload.approved) {
                setters.setWorkflowPhase('using_tools');
              }
              break;
            }

            case 'channelPermissionRequest': {
              break;
            }

            case 'workflowCompleted': {
              const nextStatus =
                payload.reason === 'cancelled' ? 'paused' : 'idle';
              setters.setWorkflowStatus(nextStatus);
              setters.setSession((prev) =>
                prev ? { ...prev, status: nextStatus } : null,
              );
              setters.setWorkflowPhase('idle');
              logger.info('Workflow phase: idle', {
                sessionId,
                reason: payload.reason,
                status: nextStatus,
              });
              break;
            }
          }
        });

        const response = await openAgentSession(sessionId);
        const sessionMetadata = response.session;

        if (!isMounted) return;

        let assistant: import('@/models/chat').Assistant | undefined;
        if (sessionMetadata.agentConfig) {
          try {
            assistant = JSON.parse(sessionMetadata.agentConfig);
          } catch (e) {
            logger.error('Failed to parse agent config', e);
          }
        }

        const sessionData: AgentSession = {
          id: sessionMetadata.id,
          name: sessionMetadata.name,
          status: sessionMetadata.status,
          model: sessionMetadata.model,
          provider: sessionMetadata.provider,
          assistant,
          createdAt: new Date(sessionMetadata.createdAt),
          updatedAt: sessionMetadata.updatedAt
            ? new Date(sessionMetadata.updatedAt)
            : undefined,
          lastViewedAt: sessionMetadata.lastViewedAt
            ? new Date(sessionMetadata.lastViewedAt)
            : undefined,
          lastMessageAt: sessionMetadata.lastMessageAt
            ? new Date(sessionMetadata.lastMessageAt)
            : undefined,
          lastAttentionAt: sessionMetadata.lastAttentionAt
            ? new Date(sessionMetadata.lastAttentionAt)
            : undefined,
          lastAttentionReason: sessionMetadata.lastAttentionReason,
          yoloMode: sessionMetadata.yoloMode,
        };

        setters.setSession(sessionData);
        setters.setWorkflowStatus(sessionData.status);
        setters.setYoloModeEnabled(sessionData.yoloMode);
        setters.setMessages(response.messages.items.map(rustMessageToMessage));
        setters.setHasOlderMessages(response.messages.hasMoreBefore);
        setters.setOldestMessageCursor(response.messages.oldestCursor ?? null);
        setters.setPendingApprovals(response.pendingApprovals ?? []);
        setters.setRuntimeState((currentState) => {
          const nextState = response.runtimeState ?? HYDRATING_RUNTIME_STATE;
          return shouldApplyRuntimeState(currentState, nextState)
            ? nextState
            : currentState;
        });
        void actions.persistViewedAt().catch((err) => {
          logger.error(
            'Failed to mark session viewed during initialization',
            err,
          );
        });
      } catch (err) {
        if (!isMounted) return;
        const errorMessage = err instanceof Error ? err.message : String(err);
        logger.error('Failed to initialize session', err);
        setters.setRuntimeState(createRuntimeFailureState(errorMessage));
        setters.setError(errorMessage);
      }
    };

    initSession();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, [sessionId, actions.persistViewedAt]);

  useEffect(() => {
    const markViewedOnReturn = () => {
      if (document.visibilityState === 'hidden') {
        return;
      }
      void actions.persistViewedAt().catch((err) => {
        logger.error('Failed to persist viewed state after focus change', err);
      });
    };

    window.addEventListener('focus', markViewedOnReturn);
    document.addEventListener('visibilitychange', markViewedOnReturn);

    return () => {
      window.removeEventListener('focus', markViewedOnReturn);
      document.removeEventListener('visibilitychange', markViewedOnReturn);
    };
  }, [actions.persistViewedAt]);
}
