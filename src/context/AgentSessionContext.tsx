import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getLogger } from '../lib/logger';
import type { Message, RustMessage, Assistant } from '@/models/chat';
import { rustMessageToMessage } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { AgentSession } from '@/models/agent';

const logger = getLogger('AgentSessionContext');

export type AgentEventPayload =
  | {
      type: 'workflowStarted';
      sessionId: string;
    }
  | {
      type: 'workflowCompleted';
      sessionId: string;
    }
  | {
      type: 'workflowError';
      sessionId: string;
      error: string;
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
  | 'error'; // Error occurred

// --- STATE CONTEXT ---
interface AgentSessionStateContextValue {
  session: AgentSession | null;
  messages: Message[];
  isSessionLoading: boolean;
  error: string | null;
  llmError: string | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
  workflowPhase: WorkflowPhase;
  initializationStep: {
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null;
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
  setError: (error: string | null) => void;

  addMessage: (message: Message) => void;
  resumeSession: () => Promise<void>;
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
  const [session, setSession] = useState<AgentSession | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSessionLoading, setIsSessionLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [llmError, setLlmError] = useState<string | null>(null);
  const [workflowStatus, setWorkflowStatus] = useState<
    'idle' | 'busy' | 'paused' | 'error'
  >('idle');
  const [workflowPhase, setWorkflowPhase] = useState<WorkflowPhase>('idle');
  const [initializationStep, setInitializationStep] = useState<{
    step: string;
    status: 'running' | 'complete' | 'error';
  } | null>(null);

  /**
   * Load messages for the current session
   */
  const loadMessages = useCallback(async (sid: string) => {
    try {
      // Load first page (large size to get all for now)
      const page = await invoke<Page<RustMessage>>('messages_get_page', {
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
              const errorMsg = payload.error;

              // Specific handling for empty LLM responses
              if (errorMsg.startsWith('EMPTY_LLM_RESPONSE:')) {
                const cleanMessage = errorMsg.replace(
                  'EMPTY_LLM_RESPONSE: ',
                  '',
                );
                setLlmError(cleanMessage);
              } else if (
                errorMsg.includes('invalid type:') ||
                errorMsg.includes('expected i64') ||
                errorMsg.includes('LLM') ||
                errorMsg.includes('MALFORMED_FUNCTION_CALL') ||
                errorMsg.toLowerCase().includes('function call') ||
                errorMsg.toLowerCase().includes('json')
              ) {
                setLlmError(errorMsg);
              } else {
                setError(errorMsg);
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
                  invoke('agent_resume_workflow', {
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
        const response = await invoke<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          agentConfig?: string;
          createdAt: number;
          updatedAt?: number;
        } | null>('agent_get_session', {
          sessionId,
        });

        if (!response) {
          throw new Error(`Session not found: ${sessionId}`);
        }

        if (!isMounted) return;

        let assistant: Assistant | undefined;
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
          assistant,
          createdAt: new Date(response.createdAt),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
        };

        setSession(sessionData);
        setWorkflowStatus(sessionData.status);

        // 2. Resume session in Rust backend (ensure active in memory)
        // This triggers proxy creation which emits InitializationStep events
        await invoke('agent_resume_session', { sessionId });

        // 3. Initialize session cache with messages in Rust
        await invoke('agent_init_session_with_messages', { sessionId });

        // 4. Load messages
        await loadMessages(sessionId);

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
  }, [sessionId, loadMessages]);

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

        const rustMessage = {
          ...message,
          createdAt: now.getTime(),
          updatedAt: now.getTime(),
        };

        await invoke('agent_send_message', {
          request: {
            sessionId: session.id,
            message: rustMessage,
          },
        });
      } catch (err) {
        logger.error('Failed to send message', err);
        throw err;
      }
    },
    [session],
  );

  /**
   * Stop the current session workflow
   */
  const stopSession = useCallback(async () => {
    if (!session) return;

    try {
      await invoke('agent_terminate_workflow', {
        sessionId: session.id,
      });
    } catch (err) {
      logger.error('Failed to stop session', err);
      setSession(null);
    }
  }, [session]);

  const resumeSession = useCallback(async () => {
    if (!session) return;

    try {
      await invoke('agent_resume_workflow', {
        sessionId: session.id,
      });
      // Status update will come via event
    } catch (err) {
      logger.error('Failed to resume session', err);
      throw err;
    }
  }, [session]);

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
    ],
  );

  const actionsValue: AgentSessionActionsContextValue = useMemo(
    () => ({
      sendMessage,
      stopSession,
      resumeSession,
      addMessage,
      setError,
    }),
    [sendMessage, stopSession, resumeSession, addMessage, setError],
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
