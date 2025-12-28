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
import { useAgentSessionState } from './AgentSessionContext';
import { useLLMService } from './LLMServiceContext';
import { getLogger } from '../lib/logger';
import type { Message } from '@/models/chat';
import { deleteMessage } from '@/lib/backend/messages';

const logger = getLogger('AgentChatContext');

/**
 * Session status from Rust backend
 */
type SessionStatus = 'Idle' | 'Busy' | 'Paused';

/**
 * Agent event from Rust backend (currently using Record<string, unknown> in listeners)
 */
// interface AgentEvent {
//   session_id: string;
//   status?: SessionStatus;
//   error?: string;
// }

// --- STATE CONTEXT ---
interface AgentChatStateContextValue {
  isLoading: boolean;
  messages: Message[];
  error: string | null;
  llmError: string | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
}

const AgentChatStateContext = createContext<
  AgentChatStateContextValue | undefined
>(undefined);

// --- ACTIONS CONTEXT ---
interface AgentChatActionsContextValue {
  /**
   * Submit a user message to the agent workflow
   * Delegates to Rust backend for orchestration
   */
  submit: (message: Message) => Promise<void>;

  /**
   * Cancel the current workflow
   */
  cancel: () => Promise<void>;

  /**
   * Retry the last failed message
   */
  retryMessage: () => Promise<void>;
}

const AgentChatActionsContext = createContext<
  AgentChatActionsContextValue | undefined
>(undefined);

interface AgentChatProviderProps {
  children: React.ReactNode;
}

/**
 * AgentChatProvider
 *
 * Simplified chat context that delegates workflow orchestration to Rust backend.
 * This replaces the complex tool execution loop in ChatContext with simple IPC calls.
 *
 * Key differences from ChatContext:
 * - No tool call orchestration (handled by Rust)
 * - No message queue management (handled by Rust)
 * - Simple submit → delegate to backend → listen for events
 */
export function AgentChatProvider({ children }: AgentChatProviderProps) {
  const { currentSession, messages: sessionMessages } = useAgentSessionState();
  const { streamingMessages, clearStreamingMessage } = useLLMService();

  // Local messages state synchronized with session messages
  // Updated through: 1) sessionMessages sync, 2) optimistic updates, 3) agent:event
  const [localMessages, setLocalMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [llmError, setLlmError] = useState<string | null>(null);
  const [workflowStatus, setWorkflowStatus] = useState<
    'idle' | 'busy' | 'paused' | 'error'
  >('idle');

  /**
   * Sync local messages with session messages (from resumeSession)
   * This happens only when sessionMessages changes (initial load)
   */
  useEffect(() => {
    logger.debug('Syncing local messages with session messages', {
      sessionId: currentSession?.id,
      sessionMessageCount: sessionMessages?.length ?? 0,
    });
    setLocalMessages(sessionMessages || []);
  }, [sessionMessages, currentSession?.id]);

  /**
   * Extract streaming message for current session
   * Memoized to prevent unnecessary effect re-runs
   */
  const currentStreamingMessage = useMemo(() => {
    if (!currentSession?.id) return undefined;
    return streamingMessages.get(currentSession.id);
  }, [currentSession?.id, streamingMessages]);

  /**
   * Merge persisted messages with streaming messages from LLMServiceContext
   * Persisted messages are added via useEffect when streaming completes
   */
  const displayMessages = useMemo(() => {
    if (!currentSession?.id) return [];

    // If there's a streaming message that's not yet in persisted messages
    if (currentStreamingMessage?.id) {
      const existsInMessages = localMessages.some(
        (m) => m.id === currentStreamingMessage.id,
      );
      if (!existsInMessages) {
        // Show streaming message alongside persisted messages
        return [...localMessages, currentStreamingMessage as Message];
      }
    }

    // Return persisted messages only
    return localMessages;
  }, [localMessages, currentStreamingMessage, currentSession?.id]);

  /**
   * Detect streaming completion and persist to React state
   * Implements idea.md architecture: "React owns the message stack"
   */
  useEffect(() => {
    if (!currentSession?.id || !currentStreamingMessage) return;

    // Only process when streaming explicitly completes
    if (currentStreamingMessage.isStreaming === false) {
      const messageId = currentStreamingMessage.id;

      // Guard: Skip if already in messages (race condition protection)
      const exists = localMessages.some((m) => m.id === messageId);
      if (exists) {
        logger.debug('Message already in stack, clearing streaming state', {
          sessionId: currentSession.id,
          messageId,
        });
        clearStreamingMessage(currentSession.id);
        return;
      }

      logger.info('Streaming completed, persisting message', {
        sessionId: currentSession.id,
        messageId,
        hasToolCalls: !!currentStreamingMessage.tool_calls?.length,
      });

      // Add to message stack
      setLocalMessages((prev) => [...prev, currentStreamingMessage as Message]);

      // Clear streaming state to prevent duplicate display
      clearStreamingMessage(currentSession.id);
    }
  }, [
    currentStreamingMessage,
    currentSession?.id,
    clearStreamingMessage,
    localMessages,
  ]);

  /**
   * Listen for agent events from Rust backend
   */
  useEffect(() => {
    if (!currentSession?.id) return;

    let unlisten: (() => void) | undefined;

    const setupListeners = async () => {
      logger.info('Setting up agent event listeners', {
        sessionId: currentSession.id,
      });

      // Listen for workflow status changes and message events
      unlisten = await listen<Record<string, unknown>>(
        'agent:event',
        async (event) => {
          const payload = event.payload;
          const eventType = payload.type as string;
          // Rust uses camelCase serialization (serde rename_all = "camelCase")
          const sessionId = (payload.sessionId || payload.session_id) as string;

          logger.info('🎯 Agent event received (BEFORE session filter)', {
            eventType,
            payloadSessionId: sessionId,
            currentSessionId: currentSession.id,
            allPayloadKeys: Object.keys(payload),
          });

          // Only process events for current session
          if (sessionId !== currentSession.id) {
            logger.warn('⚠️ Event session ID mismatch, ignoring event', {
              eventSessionId: sessionId,
              currentSessionId: currentSession.id,
              eventType,
            });
            return;
          }

          logger.debug('Received agent event (AFTER session filter)', {
            sessionId,
            eventType,
          });

          // Handle different event types
          // IMPORTANT: Rust serde uses camelCase (StatusChanged → statusChanged)
          if (eventType === 'statusChanged') {
            const status = payload.status as SessionStatus;
            if (status === 'Idle') {
              setWorkflowStatus('idle');
              setIsLoading(false);
            } else if (status === 'Busy') {
              setWorkflowStatus('busy');
              setIsLoading(true);
            } else if (status === 'Paused') {
              setWorkflowStatus('paused');
              setIsLoading(false);
            }
          } else if (eventType === 'workflowError') {
            setWorkflowStatus('error');
            setIsLoading(false);
            const errorMsg = (payload.error as string) ?? 'Unknown error';

            // Distinguish LLM errors from workflow errors
            if (
              errorMsg.includes('invalid type:') ||
              errorMsg.includes('expected i64') ||
              errorMsg.includes('LLM')
            ) {
              setLlmError(errorMsg);
              logger.error('LLM error detected', { error: errorMsg });
            } else {
              setError(errorMsg);
            }
          } else if (eventType === 'messageAdded') {
            logger.info('📨 MessageAdded event received from Rust', {
              sessionId: currentSession.id,
              hasPayload: !!payload.message,
              payloadKeys: payload.message ? Object.keys(payload.message) : [],
            });

            // ✅ Handle messages from Rust (includes full message object)
            const rawMessage = payload.message as Record<string, unknown>;

            if (!rawMessage) {
              logger.warn('MessageAdded event missing payload', { payload });
              return;
            }

            logger.debug('Raw message received', {
              id: rawMessage.id,
              role: rawMessage.role,
              toolCallId: rawMessage.toolCallId || rawMessage.tool_call_id,
              contentLength: (rawMessage.content as unknown[])?.length,
            });

            // Normalize field names (defensive, serde should handle camelCase conversion)
            // Rust's serde(rename_all = "camelCase") should already convert fields
            const newMessage: Message = {
              ...(rawMessage as unknown as Message),
              sessionId: (rawMessage.sessionId ||
                rawMessage.session_id) as string,
              tool_calls: rawMessage.toolCalls || rawMessage.tool_calls,
              tool_call_id: (rawMessage.toolCallId ||
                rawMessage.tool_call_id) as string | undefined,
              tool_use: rawMessage.toolUse || rawMessage.tool_use,
              is_streaming: rawMessage.isStreaming ?? rawMessage.is_streaming,
              thinking_signature: (rawMessage.thinkingSignature ||
                rawMessage.thinking_signature) as string | undefined,
              assistant_id: (rawMessage.assistantId ||
                rawMessage.assistant_id) as string | undefined,
              created_at: rawMessage.createdAt || rawMessage.created_at,
              updated_at: rawMessage.updatedAt || rawMessage.updated_at,
            } as Message;

            logger.info('Message normalized', {
              sessionId: currentSession.id,
              messageId: newMessage.id,
              role: newMessage.role,
              toolCallId: newMessage.tool_call_id,
            });

            // Add to messages if not already present (deduplication)
            setLocalMessages((prev) => {
              const exists = prev.some((m) => m.id === newMessage.id);
              if (exists) {
                logger.warn('⚠️ Message already in state, skipping', {
                  messageId: newMessage.id,
                  existingCount: prev.length,
                });
                return prev;
              }

              logger.info('✅ Adding message to React state', {
                messageId: newMessage.id,
                role: newMessage.role,
                previousCount: prev.length,
                newCount: prev.length + 1,
              });

              return [...prev, newMessage];
            });
          } else if (eventType === 'workflowCompleted') {
            // Workflow completed - messages already in React state per idea.md
            setWorkflowStatus('idle');
            setIsLoading(false);

            logger.info('Workflow completed', {
              sessionId: currentSession.id,
              messageCount: localMessages.length,
            });

            // No DB reload needed - React owns message state (idea.md architecture)
            // Tool results and assistant responses already added via streaming
          }
        },
      );

      logger.info('Agent event listeners registered');
    };

    setupListeners();

    return () => {
      if (unlisten) {
        unlisten();
        logger.info('Agent event listeners cleaned up');
      }
    };
  }, [currentSession?.id]);

  /**
   * Submit a user message to the agent workflow
   * This delegates to Rust backend via agent_send_message command
   */
  const submit = useCallback(
    async (message: Message) => {
      if (!currentSession?.id) {
        logger.error('Cannot submit: no active session');
        return;
      }

      logger.info('Submitting message to agent workflow', {
        sessionId: currentSession.id,
        messageId: message.id,
      });

      try {
        setIsLoading(true);
        setError(null);

        // ✅ Optimistic update - add user message immediately (idea.md pattern)
        setLocalMessages((prev) => [...prev, message]);

        // Convert Date objects to Unix timestamps for Rust backend
        // Safety net: provide fallback timestamps if missing (prevents Rust deserialization error)
        const now = Date.now();
        const messageForRust = {
          ...message,
          createdAt:
            message.createdAt instanceof Date
              ? message.createdAt.getTime()
              : message.createdAt || now,
          updatedAt:
            message.updatedAt instanceof Date
              ? message.updatedAt.getTime()
              : message.updatedAt ||
                (message.createdAt instanceof Date
                  ? message.createdAt.getTime()
                  : message.createdAt) ||
                now,
        };

        // Delegate to Rust backend (Rust will save the message to DB)
        await invoke('agent_send_message', {
          request: {
            sessionId: currentSession.id,
            message: messageForRust,
          },
        });

        logger.info('Message submitted successfully', {
          sessionId: currentSession.id,
          messageId: message.id,
        });
      } catch (err) {
        logger.error('Failed to submit message', err);
        setError(err instanceof Error ? err.message : String(err));
        setIsLoading(false);

        // ✅ Rollback on error
        setLocalMessages((prev) => prev.filter((m) => m.id !== message.id));
      }
    },
    [currentSession?.id],
  );

  /**
   * Cancel the current workflow
   */
  const cancel = useCallback(async () => {
    if (!currentSession?.id) {
      logger.error('Cannot cancel: no active session');
      return;
    }

    logger.info('Cancelling workflow', { sessionId: currentSession.id });

    try {
      await invoke('agent_terminate_workflow', {
        sessionId: currentSession.id,
      });

      setIsLoading(false);
      setWorkflowStatus('idle');

      logger.info('Workflow cancelled successfully');
    } catch (err) {
      logger.error('Failed to cancel workflow', err);
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [currentSession?.id]);

  /**
   * Retry the last failed message
   */
  const retryMessage = useCallback(async () => {
    if (!currentSession?.id) {
      logger.error('Cannot retry: no active session');
      return;
    }

    // Find the last user message
    const lastUserMessage = [...localMessages]
      .reverse()
      .find((msg) => msg.role === 'user');

    if (!lastUserMessage) {
      logger.warn('No user message to retry');
      return;
    }

    logger.info('Retrying last message', {
      sessionId: currentSession.id,
      messageId: lastUserMessage.id,
    });

    // Delete any subsequent messages (including failed assistant responses)
    const messageIndex = localMessages.findIndex(
      (msg) => msg.id === lastUserMessage.id,
    );
    const messagesToDelete = localMessages.slice(messageIndex + 1);

    for (const msg of messagesToDelete) {
      await deleteMessage(msg.id);
    }

    // Re-submit the user message (Date conversion handled by submit function)
    await submit(lastUserMessage);
  }, [currentSession?.id, localMessages, submit]);

  // Combine state values
  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isLoading,
      messages: displayMessages, // Use merged messages with streaming
      error,
      llmError,
      workflowStatus,
    }),
    [isLoading, displayMessages, error, llmError, workflowStatus],
  );

  // Combine action values
  const actionsValue: AgentChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel,
      retryMessage,
    }),
    [submit, cancel, retryMessage],
  );

  return (
    <AgentChatStateContext.Provider value={stateValue}>
      <AgentChatActionsContext.Provider value={actionsValue}>
        {children}
      </AgentChatActionsContext.Provider>
    </AgentChatStateContext.Provider>
  );
}

/**
 * Hook to access agent chat state
 */
export function useAgentChatState(): AgentChatStateContextValue {
  const context = useContext(AgentChatStateContext);
  if (!context) {
    throw new Error('useAgentChatState must be used within AgentChatProvider');
  }
  return context;
}

/**
 * Hook to access agent chat actions
 */
export function useAgentChatActions(): AgentChatActionsContextValue {
  const context = useContext(AgentChatActionsContext);
  if (!context) {
    throw new Error(
      'useAgentChatActions must be used within AgentChatProvider',
    );
  }
  return context;
}

/**
 * Convenience hook to access both state and actions
 */
export function useAgentChat() {
  const state = useAgentChatState();
  const actions = useAgentChatActions();

  return {
    ...state,
    ...actions,
  };
}
