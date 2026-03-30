import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import { rustMessageToMessage } from '@/models/chat';
import type { AgentSession } from '@/models/agent';
import type { AgentResponse, AgentSessionMetadata } from '@/models/agent-ipc';
import type { AgentEventPayload } from './types';
import { buildMessageError } from './utils';
import type { useAgentSessionState } from './useAgentSessionState';

const logger = getLogger('AgentSessionEvents');

export function useAgentSessionEvents(
  sessionId: string,
  stateProps: ReturnType<typeof useAgentSessionState>,
  actions: {
    loadMessages: (sid: string) => Promise<void>;
    persistViewedAt: (viewedAt?: Date) => Promise<void>;
  },
) {
  const { setters, refs } = stateProps;

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isMounted = true;

    const initSession = async () => {
      logger.info('Initializing agent session', { sessionId });
      setters.setIsSessionLoading(true);
      setters.setError(null);
      setters.setInitializationStep({
        step: 'Starting session...',
        status: 'running',
      });

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

          logger.debug('Agent session event received', {
            type: payload.type,
            sessionId,
          });

          switch (payload.type) {
            case 'initializationStep': {
              const rawStatus = payload.status;
              const isValidStatus =
                rawStatus === 'running' ||
                rawStatus === 'complete' ||
                rawStatus === 'error';
              const safeStatus: 'running' | 'complete' | 'error' = isValidStatus
                ? rawStatus
                : 'error';

              if (!isValidStatus) {
                logger.warn(
                  'Received invalid initialization status from backend',
                  { sessionId, rawStatus },
                );
              }

              setters.setInitializationStep({
                step: payload.step,
                status: safeStatus,
              });
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
              setters.setIsSessionLoading(false);
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
              setters.setWorkflowStatus('idle');
              setters.setWorkflowPhase('idle');
              setters.setIsSessionLoading(false);
              logger.info('Workflow phase: idle');
              break;
            }
          }
        });

        const response = await safeInvoke<AgentSessionMetadata | null>(
          'agent_get_session',
          { sessionId },
        );

        if (!response) {
          throw new Error(`Session not found: ${sessionId}`);
        }

        if (!isMounted) return;

        let assistant: import('@/models/chat').Assistant | undefined;
        if (response.agentConfig) {
          try {
            assistant = JSON.parse(response.agentConfig);
          } catch (e) {
            logger.error('Failed to parse agent config', e);
          }
        }

        const sessionData: AgentSession = {
          id: response.id,
          name: response.name,
          status: response.status,
          model: response.model,
          provider: response.provider,
          assistant,
          createdAt: new Date(response.createdAt),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
          lastViewedAt: response.lastViewedAt
            ? new Date(response.lastViewedAt)
            : undefined,
          lastMessageAt: response.lastMessageAt
            ? new Date(response.lastMessageAt)
            : undefined,
          lastAttentionAt: response.lastAttentionAt
            ? new Date(response.lastAttentionAt)
            : undefined,
          lastAttentionReason: response.lastAttentionReason,
          yoloMode: response.yoloMode,
        };

        setters.setSession(sessionData);
        setters.setWorkflowStatus(sessionData.status);
        setters.setYoloModeEnabled(sessionData.yoloMode);

        await safeInvoke<AgentSessionMetadata>('agent_resume_session', {
          sessionId,
        });
        await safeInvoke<AgentResponse>('agent_init_session_with_messages', {
          sessionId,
        });

        await actions.loadMessages(sessionId);
        void actions.persistViewedAt().catch((err) => {
          logger.error(
            'Failed to mark session viewed during initialization',
            err,
          );
        });

        if (isMounted) setters.setIsSessionLoading(false);
      } catch (err) {
        if (!isMounted) return;
        const errorMessage = err instanceof Error ? err.message : String(err);
        logger.error('Failed to initialize session', err);
        setters.setError(errorMessage);
        setters.setIsSessionLoading(false);
      }
    };

    initSession();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, [sessionId, actions.loadMessages, actions.persistViewedAt]);

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
