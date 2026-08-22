import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { listen } from '@tauri-apps/api/event';
import { openAgentSession } from '@/lib/backend/agent-commands';
import { getAssistant } from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';
import { mapSessionMetadataToAgentSession } from '@/lib/session-metadata';
import {
  rustMessageToMessage,
  type Assistant,
  type Message,
} from '@/models/chat';
import type { AgentEventPayload, PendingApproval } from './types';
import { buildMessageError, syncSessionMetadataFromBackend } from './utils';
import type { useAgentSessionState } from './useAgentSessionState';
import type {
  AgentOpenSessionResponse,
  SessionRuntimeState,
} from '@/models/agent-ipc';
import { notifyRuntimeStateErrors } from './notifyRuntimeStateErrors';
import {
  getOpenSessionView,
  invalidateOpenSessionView,
  isWarmOpenSessionView,
  putOpenSessionView,
} from './openSessionViewCache';
import {
  mergeOpenSessionMessages,
  mergePendingApprovals,
} from './mergeOpenSessionState';
import {
  applyWorkflowInactiveCleanup,
  isInactiveWorkflowStatus,
  stripMessageStreamingFlags,
} from './workflow-inactive-cleanup';
import {
  pickRuntimeState,
} from './runtimeStateApply';

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

type SessionStateSetters = ReturnType<typeof useAgentSessionState>['setters'];

function applyOpenSessionPayload(
  response: AgentOpenSessionResponse,
  setters: SessionStateSetters,
  options: {
    assistant?: Assistant;
    clearStreamingOnInactive: () => void;
    sessionIdForErrors: string;
    /** When true, preserve event-arrived rows that the open snapshot missed. */
    mergeWithLiveState?: boolean;
  },
): SessionRuntimeState {
  const sessionMetadata = response.session;
  const sessionData = mapSessionMetadataToAgentSession(
    sessionMetadata,
    response.pendingApprovals?.length ?? 0,
    options.assistant,
  );

  setters.setSession(sessionData);
  setters.setWorkflowStatus(sessionData.status);
  setters.applyExecutionMode(sessionData.executionMode);

  const hydratedMessages = response.messages.items.map(rustMessageToMessage);
  const preparedIncoming = isInactiveWorkflowStatus(sessionData.status)
    ? stripMessageStreamingFlags(hydratedMessages)
    : hydratedMessages;
  const incomingApprovals = response.pendingApprovals ?? [];

  if (options.mergeWithLiveState) {
    setters.setMessages((previous: Message[]) =>
      mergeOpenSessionMessages(previous, preparedIncoming),
    );
    setters.setPendingApprovals((previous: PendingApproval[]) =>
      mergePendingApprovals(previous, incomingApprovals),
    );
  } else {
    setters.setMessages(preparedIncoming);
    setters.setPendingApprovals(incomingApprovals);
  }

  if (isInactiveWorkflowStatus(sessionData.status)) {
    options.clearStreamingOnInactive();
  }
  setters.setHasOlderMessages(response.messages.hasMoreBefore);
  setters.setOldestMessageCursor(response.messages.oldestCursor ?? null);

  const nextRuntimeState = response.runtimeState ?? HYDRATING_RUNTIME_STATE;
  // useState updaters run synchronously — capture the reconciled value for the
  // open-view cache so remounts paint Ready when events already advanced past open().
  let previousRuntimeState: SessionRuntimeState | null = null;
  let appliedRuntimeState = nextRuntimeState;
  setters.setRuntimeState((currentState) => {
    const picked = pickRuntimeState(currentState, nextRuntimeState);
    if (picked === nextRuntimeState) {
      previousRuntimeState = currentState;
    }
    appliedRuntimeState = picked;
    return picked;
  });
  if (previousRuntimeState) {
    notifyRuntimeStateErrors(
      previousRuntimeState,
      nextRuntimeState,
      options.sessionIdForErrors,
    );
  }
  return appliedRuntimeState;
}

export function useAgentSessionEvents(
  sessionId: string,
  stateProps: ReturnType<typeof useAgentSessionState>,
  actions: {
    persistViewedAt: (viewedAt?: Date) => Promise<void>;
    /** Clears LLM streaming placeholders when the workflow becomes inactive. */
    clearStreamingMessage: (sessionId: string) => void;
    /**
     * Keep-alive inactive panes still receive agent events, but must not mark
     * the session viewed on window focus / visibility changes.
     */
    isActive?: boolean;
  },
) {
  const navigate = useNavigate();
  const { setters, refs } = stateProps;
  const isActive = actions.isActive ?? true;
  const clearStreamingOnInactive = () =>
    applyWorkflowInactiveCleanup({
      sessionId,
      clearStreamingMessage: actions.clearStreamingMessage,
      setMessages: setters.setMessages,
    });

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isMounted = true;
    let liveMutationEpoch = 0;

    const initSession = async () => {
      logger.info('Initializing agent session', { sessionId });
      setters.setError(null);
      setters.setPreflightTokenMetrics(null);

      const cachedView = getOpenSessionView(sessionId);
      if (cachedView && isWarmOpenSessionView(cachedView)) {
        // Paint last-known ready UI immediately; openAgentSession reconciles below.
        applyOpenSessionPayload(cachedView, setters, {
          clearStreamingOnInactive,
          sessionIdForErrors: sessionId,
        });
      } else {
        setters.setRuntimeState(HYDRATING_RUNTIME_STATE);
      }

      try {
        const unlistenFn = await listen<AgentEventPayload>(
          'agent:event',
          (event) => {
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
                let previousState: SessionRuntimeState | null = null;
                setters.setRuntimeState((currentState) => {
                  const picked = pickRuntimeState(currentState, nextState);
                  if (picked === nextState && currentState !== nextState) {
                    previousState = currentState;
                  }
                  return picked;
                });
                if (previousState) {
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
                liveMutationEpoch += 1;
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
                liveMutationEpoch += 1;
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
                liveMutationEpoch += 1;
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
                liveMutationEpoch += 1;
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
                  liveMutationEpoch += 1;
                  setters.clearSessionHistory();
                  invalidateOpenSessionView(sessionId);
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
          },
        );

        if (!isMounted) {
          unlistenFn();
          return;
        }
        unlisten = unlistenFn;

        const mutationEpochAtOpen = liveMutationEpoch;
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
          // Cache under the storage id so the remount can warm-paint Ready when
          // open() already returned a ready snapshot (alias listeners drop
          // storage-id runtime events).
          putOpenSessionView(resolvedSessionId, {
            ...response,
            runtimeState: response.runtimeState ?? HYDRATING_RUNTIME_STATE,
          });
          navigate(`/agent/${resolvedSessionId}`, { replace: true });
          return;
        }

        if (!isMounted) return;

        const assistant = response.session.assistantId
          ? await getAssistant(response.session.assistantId)
          : undefined;
        if (!isMounted) return;

        const appliedRuntimeState = applyOpenSessionPayload(response, setters, {
          assistant,
          clearStreamingOnInactive,
          sessionIdForErrors: sessionId,
          // Only merge when agent:event mutated transcript/approvals during open;
          // otherwise replace so warm-cache paint can be fully reconciled from DB.
          mergeWithLiveState: liveMutationEpoch !== mutationEpochAtOpen,
        });
        putOpenSessionView(sessionId, {
          ...response,
          runtimeState: appliedRuntimeState,
        });

        void actions.persistViewedAt().catch((err) => {
          logger.error(
            'Failed to mark session viewed during initialization',
            err,
          );
        });
      } catch (err) {
        if (!isMounted) return;
        invalidateOpenSessionView(sessionId);
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
    if (!isActive) {
      return;
    }

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
  }, [actions.persistViewedAt, isActive]);
}
