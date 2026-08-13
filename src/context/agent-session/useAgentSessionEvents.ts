import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { openAgentSession } from '@/lib/backend/agent-commands';
import { getAssistant } from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';
import { mapSessionMetadataToAgentSession } from '@/lib/session-metadata';
import { rustMessageToMessage } from '@/models/chat';
import type { AgentEventPayload } from './types';
import { buildMessageError, syncSessionMetadataFromBackend } from './utils';
import type { useAgentSessionState } from './useAgentSessionState';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import { notifyRuntimeStateErrors } from './notifyRuntimeStateErrors';
import {
  applyWorkflowInactiveCleanup,
  isInactiveWorkflowStatus,
  stripMessageStreamingFlags,
} from './workflow-inactive-cleanup';

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
    /** Clears LLM streaming placeholders when the workflow becomes inactive. */
    clearStreamingMessage: (sessionId: string) => void;
  },
) {
  const navigate = useNavigate();
  const { setters, refs } = stateProps;
  const clearStreamingOnInactive = () =>
    applyWorkflowInactiveCleanup({
      sessionId,
      clearStreamingMessage: actions.clearStreamingMessage,
      setMessages: setters.setMessages,
    });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isMounted = true;

    const initSession = async () => {
      logger.info('Initializing agent session', { sessionId });
      setters.setError(null);
      setters.setRuntimeState(HYDRATING_RUNTIME_STATE);
      setters.setPreflightTokenMetrics(null);

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
              const nextState = payload.runtimeState;
              let applied = false;
              let previousState: SessionRuntimeState | null = null;
              setters.setRuntimeState((currentState) => {
                if (!shouldApplyRuntimeState(currentState, nextState)) {
                  return currentState;
                }
                previousState = currentState;
                applied = true;
                return nextState;
              });
              if (applied && previousState) {
                notifyRuntimeStateErrors(previousState, nextState, sessionId);
              }
              break;
            }

            case 'preflightTokenMetricsUpdated': {
              setters.setPreflightTokenMetrics(payload.metrics);
              break;
            }

            case 'workflowStarted': {
              setters.setError(null);
              setters.setLlmError(null);
              logger.info('Workflow started');
              break;
            }

            case 'statusChanged': {
              const newStatus = payload.status;
              setters.setWorkflowStatus(newStatus);
              setters.setSession((prev) =>
                prev ? { ...prev, status: newStatus } : null,
              );

              if (newStatus === 'busy' || newStatus === 'queued') {
                setters.setError(null);
                setters.setLlmError(null);
                setters.setWorkflowPhase(
                  newStatus === 'busy' ? 'thinking' : 'idle',
                );
              } else if (newStatus === 'idle') {
                setters.setWorkflowPhase('idle');
                clearStreamingOnInactive();
              } else if (newStatus === 'error') {
                setters.setWorkflowPhase('error');
                clearStreamingOnInactive();
              } else if (newStatus === 'paused') {
                clearStreamingOnInactive();
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
                    approvalKind: payload.approvalKind,
                    requestId: payload.requestId,
                    description: payload.description,
                    inputPreview: payload.inputPreview,
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

            case 'circuitBreakerTriggered': {
              logger.warn('Circuit breaker triggered', {
                toolName: payload.toolName,
                count: payload.count,
                action: payload.action,
              });
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

            case 'interactiveShellInputRequested': {
              setters.setPendingInteractiveShellPrompt({
                executionId: payload.executionId,
                prompt: payload.prompt,
                inputType: payload.inputType,
                command: payload.command,
              });
              setters.setWorkflowPhase('using_tools');
              break;
            }

            case 'interactiveShellInputResolved': {
              setters.setPendingInteractiveShellPrompt((currentPrompt) =>
                currentPrompt?.executionId === payload.executionId
                  ? null
                  : currentPrompt,
              );
              break;
            }

            case 'channelPermissionRequest': {
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
                    arguments: payload.inputPreview,
                    approvalKind: payload.approvalKind,
                    requestId: payload.requestId,
                    description: payload.description,
                    inputPreview: payload.inputPreview,
                  },
                ];
              });
              break;
            }

            case 'resourceUpdated': {
              if (
                payload.resourceType !== 'session' ||
                payload.resourceId !== sessionId
              ) {
                break;
              }

              if (payload.action === 'clear') {
                setters.clearSessionHistory();
                logger.info(
                  'Session history cleared via resourceUpdated event',
                );
                break;
              }

              if (payload.action === 'update') {
                void syncSessionMetadataFromBackend(sessionId, setters);
              }
              break;
            }

            case 'workflowCompleted': {
              // Soft cancel → paused (resume). Terminate / natural stop → idle.
              const nextStatus =
                payload.reason === 'cancelled' ? 'paused' : 'idle';
              setters.setWorkflowStatus(nextStatus);
              setters.setSession((prev) =>
                prev ? { ...prev, status: nextStatus } : null,
              );
              setters.setWorkflowPhase('idle');
              setters.setPendingInteractiveShellPrompt(null);
              clearStreamingOnInactive();
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
        if (!isMounted) return;

        // Events and further commands use storage ids. If the route still has a
        // display alias, remount on the resolved storage key.
        const resolvedSessionId = response.session.id;
        if (resolvedSessionId !== sessionId) {
          logger.info('Normalizing session route to storage id', {
            from: sessionId,
            to: resolvedSessionId,
          });
          navigate(`/agent/${resolvedSessionId}`, { replace: true });
          return;
        }

        const sessionMetadata = response.session;

        if (!isMounted) return;

        const assistant = sessionMetadata.assistantId
          ? await getAssistant(sessionMetadata.assistantId)
          : undefined;

        const sessionData = mapSessionMetadataToAgentSession(
          sessionMetadata,
          response.pendingApprovals?.length ?? 0,
          assistant,
        );

        setters.setSession(sessionData);
        setters.setWorkflowStatus(sessionData.status);
        setters.applyExecutionMode(sessionData.executionMode);
        const hydratedMessages =
          response.messages.items.map(rustMessageToMessage);
        setters.setMessages(
          isInactiveWorkflowStatus(sessionData.status)
            ? stripMessageStreamingFlags(hydratedMessages)
            : hydratedMessages,
        );
        if (isInactiveWorkflowStatus(sessionData.status)) {
          clearStreamingOnInactive();
        }
        setters.setHasOlderMessages(response.messages.hasMoreBefore);
        setters.setOldestMessageCursor(response.messages.oldestCursor ?? null);
        setters.setPendingApprovals(response.pendingApprovals ?? []);
        // Use runtimeState from response as initial state; sequence check ensures we don't overwrite newer events
        const nextRuntimeState =
          response.runtimeState ?? HYDRATING_RUNTIME_STATE;
        let previousRuntimeState: SessionRuntimeState | null = null;
        setters.setRuntimeState((currentState) => {
          if (shouldApplyRuntimeState(currentState, nextRuntimeState)) {
            previousRuntimeState = currentState;
            return nextRuntimeState;
          }
          return currentState;
        });
        if (previousRuntimeState) {
          notifyRuntimeStateErrors(
            previousRuntimeState,
            nextRuntimeState,
            sessionId,
          );
        }
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
        setters.setSession(null);
        setters.setMessages([]);
        setters.setHasOlderMessages(false);
        setters.setOldestMessageCursor(null);
        setters.setPendingApprovals([]);
        setters.applyExecutionMode('normal');
        setters.setWorkflowStatus('error');
        setters.setWorkflowPhase('error');
        setters.setLlmError(null);
        setters.setRuntimeState(createRuntimeFailureState(errorMessage));
        setters.setError(errorMessage);
      }
    };

    initSession();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, [
    sessionId,
    navigate,
    actions.persistViewedAt,
    actions.clearStreamingMessage,
  ]);

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
