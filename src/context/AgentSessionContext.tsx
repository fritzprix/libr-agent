import { safeInvoke } from '@/lib/backend/core';
import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

import { listen } from '@tauri-apps/api/event';
import { getLogger } from '../lib/logger';
import type { Message, MessageError, RustMessage } from '@/models/chat';
import { rustMessageToMessage } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { AgentSession } from '@/models/agent';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type {
  AgentRuntimeError,
  AgentSessionMetadata,
  AgentResponse,
  SendUserMessageRequest,
  WorkflowCompletionReason,
} from '@/models/agent-ipc';
import { useAgentSessionListActions } from './AgentSessionListContext';

const logger = getLogger('AgentSessionContext');

function buildMessageError(
  error: string | AgentRuntimeError,
  fallbackType: MessageError['type'] = 'AI_SERVICE_ERROR',
): MessageError {
  if (typeof error !== 'string') {
    return error;
  }

  return {
    type: fallbackType,
    displayMessage: error,
    recoverable: true,
    details: {
      originalError: error,
      timestamp: new Date().toISOString(),
    },
  };
}

export type AgentEventPayload =
  | {
      type: 'workflowStarted';
      sessionId: string;
    }
  | {
      type: 'workflowCompleted';
      sessionId: string;
      reason: WorkflowCompletionReason;
    }
  | {
      type: 'workflowError';
      sessionId: string;
      error: AgentRuntimeError;
    }
  | {
      type: 'statusChanged';
      sessionId: string;
      status: 'idle' | 'busy' | 'paused' | 'error';
    }
  | {
      type: 'messageAdded';
      sessionId: string;
      message: RustMessage;
    }
  | {
      type: 'toolExecutionStarted';
      sessionId: string;
      toolName: string;
    }
  | {
      type: 'toolExecutionCompleted';
      sessionId: string;
      toolName: string;
      success: boolean;
    }
  | {
      type: 'toolExecutionRequiresApproval';
      sessionId: string;
      toolCallId: string;
      toolName: string;
      arguments: string;
    }
  | {
      type: 'initializationStep';
      sessionId: string;
      step: string;
      status: 'running' | 'complete' | 'error';
    }
  | {
      type: 'resourceUpdated';
      resourceType: string;
      action: string;
      resourceId?: string;
    };

// Workflow phase within 'busy' status for fine-grained UI feedback
export type WorkflowPhase =
  | 'idle' // Not processing
  | 'thinking' // Waiting for LLM response to start
  | 'answering' // LLM is streaming response
  | 'using_tools' // Executing tool calls
  | 'waiting_approval' // Blocked waiting for user approval
  | 'error'; // Error occurred

export interface PendingApproval {
  toolCallId: string;
  toolName: string;
  arguments: string;
}

// --- STATE CONTEXT ---
interface AgentSessionStateContextValue {
  session: AgentSession | null;
  messages: Message[];
  isSessionLoading: boolean;
  error: MessageError | null;
  llmError: MessageError | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
  workflowPhase: WorkflowPhase;
  initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null;
  pendingApprovals: PendingApproval[];
  yoloModeEnabled: boolean;
}

const AgentSessionStateContext = createContext<
  AgentSessionStateContextValue | undefined
>(undefined);

// --- ACTIONS CONTEXT ---
interface AgentSessionActionsContextValue {
  /**
   * Send a user message to this session
   */
  sendMessage: (content: string) => Promise<void>;

  /**
   * Stop this session workflow
   */
  stopSession: () => Promise<void>;

  /**
   * Manually set error state (e.g. for client-side failures)
   */
  setError: (error: string | AgentRuntimeError | null) => void;

  addMessage: (message: Message) => void;
  resumeSession: () => Promise<void>;
  respondToToolApproval: (
    toolCallId: string,
    approved: boolean,
  ) => Promise<void>;
  toggleYoloMode: () => void;
  updateSessionConfig: (model: string, provider: string) => void;
}

const AgentSessionActionsContext = createContext<
  AgentSessionActionsContextValue | undefined
>(undefined);

interface AgentSessionProviderProps {
  children: React.ReactNode;
  sessionId: string;
}

/**
 * AgentSessionProvider
 *
 * Manages the state for a SINGLE active agent session.
 * Requires a `sessionId` prop to initialize.
 */
export function AgentSessionProvider({
  children,
  sessionId,
}: AgentSessionProviderProps) {
  const { markSessionViewed, clearPendingApproval } =
    useAgentSessionListActions();
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  const [error, setErrorState] = useState<MessageError | null>(null);
  const [llmError, setLlmError] = useState<MessageError | null>(null);
  const [workflowStatus, setWorkflowStatus] = useState<
    'idle' | 'busy' | 'paused' | 'error'
  >('idle');
  const [workflowPhase, setWorkflowPhase] = useState<WorkflowPhase>('idle');
  const [initializationStep, setInitializationStep] = useState<{
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null>(null);
  const [pendingApprovals, setPendingApprovals] = useState<PendingApproval[]>(
    [],
  );
  const [yoloModeEnabled, setYoloModeEnabled] = useState(false);
  const yoloModeRef = useRef(yoloModeEnabled);

  const setError = useCallback(
    (nextError: string | AgentRuntimeError | null) => {
      setErrorState(nextError ? buildMessageError(nextError) : null);
    },
    [],
  );

  useEffect(() => {
    yoloModeRef.current = yoloModeEnabled;
  }, [yoloModeEnabled]);

  const applyLocalViewedAt = useCallback((viewedAt: Date) => {
    setSession((prev) =>
      prev ? applyViewedAtToSession(prev, viewedAt) : prev,
    );
  }, []);

  const persistViewedAt = useCallback(
    async (viewedAt = new Date()) => {
      applyLocalViewedAt(viewedAt);
      await markSessionViewed(sessionId, viewedAt);
    },
    [applyLocalViewedAt, markSessionViewed, sessionId],
  );

  const acknowledgeSessionAttention = useCallback(
    async (viewedAt = new Date()) => {
      await persistViewedAt(viewedAt);
    },
    [persistViewedAt],
  );

  /**
   * Load messages for the current session
   */
  const loadMessages = useCallback(async (sid: string) => {
    try {
      // Load first page (large size to get all for now)
      const page = await safeInvoke<Page<RustMessage>>('messages_get_page', {
        sessionId: sid,
        page: 1,
        pageSize: 1000,
      });

      // Convert RustMessages to Messages using type-safe converter
      const msgs: Message[] = page.items.map(rustMessageToMessage);

      logger.info(`Loaded ${msgs.length} messages for session ${sid}`, {
        messageCount: msgs.length,
        firstMessage: msgs[0]?.id,
        lastMessage: msgs[msgs.length - 1]?.id,
      });

      setMessages(msgs);
    } catch (err) {
      logger.error('Failed to load messages', err);
    }
  }, []);

  /**
   * Initialize session
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let isMounted = true;

    const initSession = async () => {
      logger.info('Initializing agent session', { sessionId });
      setIsSessionLoading(true);
      setError(null);
      setInitializationStep({ step: 'Starting session...', status: 'running' });

      try {
        // 0. Setup Event Listener FIRST to catch initialization events
        unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
          if (!isMounted) return;

          const payload = event.payload;

          // Strict Session Isolation: Only process events for THIS session
          // (except for global events like resourceUpdated that don't have a sessionId)
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
                  {
                    sessionId,
                    rawStatus,
                  },
                );
              }

              setInitializationStep({
                step: payload.step,
                status: safeStatus,
              });
              // Don't set isSessionLoading here - let the main init flow control it
              // after all operations (agent_init_session_with_messages, loadMessages) complete
              break;
            }

            case 'workflowStarted': {
              setWorkflowStatus('busy');
              setWorkflowPhase('thinking');
              setError(null);
              setLlmError(null);
              logger.info('Workflow phase: thinking');
              break;
            }

            case 'statusChanged': {
              const newStatus = payload.status;
              setWorkflowStatus(newStatus);
              setSession((prev) =>
                prev ? { ...prev, status: newStatus } : null,
              );

              // ✅ Clear errors when status changes to 'busy' (e.g. on retry)
              if (newStatus === 'busy') {
                setError(null);
                setLlmError(null);
                setWorkflowPhase('thinking');
              } else if (newStatus === 'idle') {
                setWorkflowPhase('idle');
              } else if (newStatus === 'error') {
                setWorkflowPhase('error');
              }
              break;
            }

            case 'workflowError': {
              setWorkflowStatus('error');
              setIsSessionLoading(false);
              const nextError = buildMessageError(payload.error);

              if (
                nextError.type === 'MALFORMED_FUNCTION_CALL' ||
                nextError.type === 'JSON_PARSING_ERROR' ||
                nextError.type === 'EMPTY_SELECTION_ERROR'
              ) {
                setLlmError(nextError);
                setError(null);
              } else {
                setError(nextError);
                setLlmError(null);
              }
              break;
            }

            case 'messageAdded': {
              const rustMessage = payload.message;
              const newMessage = rustMessageToMessage(rustMessage);

              // Phase transition: assistant message with streaming indicates answering phase
              if (
                newMessage.role === 'assistant' &&
                newMessage.isStreaming &&
                workflowPhase === 'thinking'
              ) {
                setWorkflowPhase('answering');
                logger.info('Workflow phase: answering');
              }

              setMessages((prev) => {
                if (prev.some((m) => m.id === newMessage.id)) return prev;
                return [...prev, newMessage];
              });

              if (!newMessage.isStreaming) {
                applyLocalViewedAt(new Date(rustMessage.createdAt));
              }

              // Recurring Request Logic for Think-Only Messages
              // If the assistant sends a message with ONLY thinking (no content, no tool calls),
              // we treat it as an internal thought and automatically trigger the next turn.
              if (
                newMessage.role === 'assistant' &&
                !newMessage.isStreaming && // Only valid for completed messages
                newMessage.thinking && // Has thinking
                (!newMessage.content || newMessage.content.length === 0) && // No visible content
                (!newMessage.tool_calls || newMessage.tool_calls.length === 0) // No tool calls
              ) {
                logger.info(
                  'Detected Think-Only message, triggering recurring request',
                  {
                    messageId: newMessage.id,
                  },
                );

                // Use resume_workflow to trigger the next turn
                // We use setTimeout to allow the UI to render the thinking bubble state first
                // and to avoid immediate state thrashing
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
              setWorkflowPhase('using_tools');
              logger.info('Workflow phase: using_tools', {
                toolName: payload.toolName,
              });
              break;
            }

            case 'toolExecutionRequiresApproval': {
              if (yoloModeRef.current) {
                logger.info(
                  'YOLO Mode enabled: Auto-approving tool execution',
                  {
                    toolName: payload.toolName,
                    toolCallId: payload.toolCallId,
                  },
                );

                // Auto-approve the tool call
                safeInvoke<AgentResponse>('agent_respond_tool_approval', {
                  sessionId,
                  toolCallId: payload.toolCallId,
                  approved: true,
                }).catch((err) => {
                  logger.error('Failed to auto-approve tool in YOLO mode', err);
                });

                // Set phase to using_tools since we are immediately continuing
                setWorkflowPhase('using_tools');
                break;
              }

              setWorkflowPhase('waiting_approval');
              setPendingApprovals((prev) => {
                // Prevent duplicate entries on session resume
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
              // Stay in using_tools or switch back to thinking/answering depending on flow
              // Usually handled by subsequent messages or status changes
              break;
            }

            case 'workflowCompleted': {
              setWorkflowStatus('idle');
              setWorkflowPhase('idle');
              setIsSessionLoading(false);
              logger.info('Workflow phase: idle');
              break;
            }
          }
        });

        // 1. Get session metadata
        const response = await safeInvoke<AgentSessionMetadata | null>(
          'agent_get_session',
          {
            sessionId,
          },
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

        setSession(sessionData);
        setWorkflowStatus(sessionData.status);
        setYoloModeEnabled(sessionData.yoloMode);
        // 2. Resume session in Rust backend (ensure active in memory)
        // This triggers proxy creation which emits InitializationStep events
        await safeInvoke<AgentSessionMetadata>('agent_resume_session', {
          sessionId,
        });

        // 3. Initialize session cache with messages in Rust
        await safeInvoke<AgentResponse>('agent_init_session_with_messages', {
          sessionId,
        });

        // 4. Load messages
        await loadMessages(sessionId);
        void persistViewedAt().catch((err) => {
          logger.error(
            'Failed to mark session viewed during initialization',
            err,
          );
        });

        // If we get here without error, we are mostly done.
        // The event listener handles the "complete" step or we can just set loading false
        // But let's rely on the event or a final check.
        // If session was already active, we might not get events.
        if (isMounted) setIsSessionLoading(false);
      } catch (err) {
        if (!isMounted) return;
        const errorMessage = err instanceof Error ? err.message : String(err);
        logger.error('Failed to initialize session', err);
        setError(errorMessage);
        setIsSessionLoading(false);
      }
    };

    initSession();

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, [sessionId, loadMessages, persistViewedAt]);

  useEffect(() => {
    const markViewedOnReturn = () => {
      if (document.visibilityState === 'hidden') {
        return;
      }

      void persistViewedAt().catch((err) => {
        logger.error('Failed to persist viewed state after focus change', err);
      });
    };

    window.addEventListener('focus', markViewedOnReturn);
    document.addEventListener('visibilitychange', markViewedOnReturn);

    return () => {
      window.removeEventListener('focus', markViewedOnReturn);
      document.removeEventListener('visibilitychange', markViewedOnReturn);
    };
  }, [persistViewedAt]);

  const addMessage = useCallback((message: Message) => {
    setMessages((prev) => {
      if (prev.some((m) => m.id === message.id)) return prev;
      return [...prev, message];
    });
  }, []);

  /**
   * Send a user message to the current session
   */
  const sendMessage = useCallback(
    async (content: string) => {
      if (!session) {
        throw new Error('No active session initialized');
      }

      try {
        const messageId = `msg_${Date.now()}`;
        const now = new Date();
        const message: Message = {
          id: messageId,
          sessionId: session.id,
          threadId: session.id,
          role: 'user',
          content: [{ type: 'text', text: content }],
          createdAt: now,
          updatedAt: now,
        };

        const rustMessage: RustMessage = {
          id: message.id,
          sessionId: message.sessionId,
          role: message.role,
          content: message.content,
          toolCalls: message.tool_calls,
          toolCallId: message.tool_call_id,
          isStreaming: message.isStreaming,
          thinking: message.thinking,
          thinkingSignature: message.thinkingSignature,
          thinkingTime: message.thinkingTime,
          assistantId: message.assistantId,
          attachments: message.attachments,
          toolUse: message.tool_use,
          createdAt: now.getTime(),
          updatedAt: now.getTime(),
          source: message.source,
          error: message.error,
          metadata: message.metadata,
        };

        const request: SendUserMessageRequest = {
          sessionId: session.id,
          message: rustMessage,
        };

        await safeInvoke<AgentResponse>('agent_send_message', { request });
        await acknowledgeSessionAttention(now);
      } catch (err) {
        logger.error('Failed to send message', err);
        throw err;
      }
    },
    [acknowledgeSessionAttention, session],
  );

  /**
   * Stop the current session workflow
   */
  const stopSession = useCallback(async () => {
    if (!session) return;

    try {
      await safeInvoke<AgentResponse>('agent_terminate_workflow', {
        sessionId: session.id,
      });
    } catch (err) {
      // Log the error but do NOT discard the session reference.
      // Clearing session here would orphan the backend session (still alive)
      // while leaving the frontend with no handle to retry or cancel.
      logger.error('Failed to stop session', err);
    }
  }, [session]);

  const resumeSession = useCallback(async () => {
    if (!session) return;

    try {
      await safeInvoke<AgentResponse>('agent_resume_workflow', {
        sessionId: session.id,
      });
      await acknowledgeSessionAttention();
      // Status update will come via event
    } catch (err) {
      logger.error('Failed to resume session', err);
      throw err;
    }
  }, [acknowledgeSessionAttention, session]);

  const respondToToolApproval = useCallback(
    async (toolCallId: string, approved: boolean) => {
      if (!session) return;
      try {
        await safeInvoke<AgentResponse>('agent_respond_tool_approval', {
          sessionId: session.id,
          toolCallId,
          approved,
        });

        // Remove from pending list
        setPendingApprovals((prev) =>
          prev.filter((p) => p.toolCallId !== toolCallId),
        );
        clearPendingApproval(session.id, toolCallId);
        await acknowledgeSessionAttention();
      } catch (err) {
        logger.error('Failed to respond to tool approval', err);
        throw err;
      }
    },
    [acknowledgeSessionAttention, clearPendingApproval, session],
  );

  const toggleYoloMode = useCallback(async () => {
    const newVal = !yoloModeEnabled;
    try {
      await safeInvoke<void>('agent_set_yolo_mode', {
        sessionId,
        enabled: newVal,
      });
      setYoloModeEnabled(newVal);
      logger.info(`YOLO mode ${newVal ? 'enabled' : 'disabled'}`);

      // If turned ON, auto-approve any currently pending approvals
      // (This part is still useful for immediate UI feedback on pending items)
      if (newVal && pendingApprovals.length > 0) {
        logger.info('Auto-approving pending tools due to YOLO toggle', {
          count: pendingApprovals.length,
        });
        const approvalsToClear = [...pendingApprovals];
        approvalsToClear.forEach((p) => {
          void safeInvoke<AgentResponse>('agent_respond_tool_approval', {
            sessionId,
            toolCallId: p.toolCallId,
            approved: true,
          }).catch((err) => {
            logger.error('Failed to auto-approve tool upon YOLO toggle', err);
          });
          clearPendingApproval(sessionId, p.toolCallId);
        });
        setPendingApprovals([]);
        setWorkflowPhase('using_tools');
        await acknowledgeSessionAttention();
      }
    } catch (err) {
      logger.error('Failed to toggle YOLO mode on backend', err);
    }
  }, [
    acknowledgeSessionAttention,
    clearPendingApproval,
    pendingApprovals,
    sessionId,
    yoloModeEnabled,
  ]);

  const updateSessionConfig = useCallback((model: string, provider: string) => {
    setSession((prev) => (prev ? { ...prev, model, provider } : null));
  }, []);

  const stateValue: AgentSessionStateContextValue = useMemo(
    () => ({
      session,
      messages,
      isSessionLoading,
      error,
      llmError,
      workflowStatus,
      workflowPhase,
      initializationStep,
      pendingApprovals,
      yoloModeEnabled,
    }),
    [
      session,
      messages,
      isSessionLoading,
      error,
      llmError,
      workflowStatus,
      workflowPhase,
      initializationStep,
      pendingApprovals,
      yoloModeEnabled,
    ],
  );

  const actionsValue: AgentSessionActionsContextValue = useMemo(
    () => ({
      sendMessage,
      stopSession,
      resumeSession,
      addMessage,
      setError,
      respondToToolApproval,
      toggleYoloMode,
      updateSessionConfig,
    }),
    [
      sendMessage,
      stopSession,
      resumeSession,
      addMessage,
      setError,
      respondToToolApproval,
      toggleYoloMode,
      updateSessionConfig,
    ],
  );

  return (
    <AgentSessionStateContext.Provider value={stateValue}>
      <AgentSessionActionsContext.Provider value={actionsValue}>
        {children}
      </AgentSessionActionsContext.Provider>
    </AgentSessionStateContext.Provider>
  );
}

/**
 * Hook to access agent session state
 */
export function useAgentSessionState(): AgentSessionStateContextValue {
  const context = useContext(AgentSessionStateContext);
  if (!context) {
    throw new Error(
      'useAgentSessionState must be used within AgentSessionProvider',
    );
  }
  return context;
}

/**
 * Hook to access agent session actions
 */
export function useAgentSessionActions(): AgentSessionActionsContextValue {
  const context = useContext(AgentSessionActionsContext);
  if (!context) {
    throw new Error(
      'useAgentSessionActions must be used within AgentSessionProvider',
    );
  }
  return context;
}

/**
 * Convenience hook to access both state and actions
 */
export function useAgentSession() {
  const state = useAgentSessionState();
  const actions = useAgentSessionActions();

  return {
    ...state,
    ...actions,
  };
}
