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
import {
  getMessagesPageForSession,
  deleteMessage,
} from '@/lib/backend/messages';

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
  const { currentSession } = useAgentSessionState();
  const { streamingMessages } = useLLMService();

  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [workflowStatus, setWorkflowStatus] = useState<
    'idle' | 'busy' | 'paused' | 'error'
  >('idle');

  /**
   * Merge persisted messages with streaming messages from LLMServiceContext
   * Persisted messages are added via useEffect when streaming completes
   */
  const displayMessages = useMemo(() => {
    if (!currentSession?.id) return [];

    const streamingMessage = streamingMessages.get(currentSession.id);

    // If there's a streaming message that's not yet in persisted messages
    if (streamingMessage?.id) {
      const existsInMessages = messages.some(
        (m) => m.id === streamingMessage.id,
      );
      if (!existsInMessages) {
        // Show streaming message alongside persisted messages
        return [...messages, streamingMessage as Message];
      }
    }

    // Return persisted messages only
    return messages;
  }, [messages, streamingMessages, currentSession?.id]);

  /**
   * When streaming completes (isStreaming: false), add the message to persisted state
   * This implements idea.md: "useAgentSession -> useAgentSession: updateMessage(messages)"
   */
  useEffect(() => {
    if (!currentSession?.id) return;

    const streamingMessage = streamingMessages.get(currentSession.id);

    // Only add when streaming is explicitly completed (isStreaming: false)
    if (streamingMessage?.id && streamingMessage.isStreaming === false) {
      // Check if already in messages
      const existsInMessages = messages.some(
        (m) => m.id === streamingMessage.id,
      );

      if (!existsInMessages) {
        // Add to messages array (React owns the state per idea.md)
        logger.info('Adding completed streaming message to persisted state', {
          messageId: streamingMessage.id,
          sessionId: currentSession.id,
          hasToolCalls: !!streamingMessage.tool_calls?.length,
        });

        setMessages((prev) => [...prev, streamingMessage as Message]);
      }
    }
  }, [streamingMessages, currentSession?.id, messages]);

  /**
   * Load messages from Rust SQLite when session changes
   */
  useEffect(() => {
    if (!currentSession?.id) {
      setMessages([]);
      return;
    }

    const loadMessages = async () => {
      try {
        const page = await getMessagesPageForSession(
          currentSession.id,
          currentSession.id, // threadId = sessionId for top-level thread
          1,
          1000,
        );
        setMessages(page.items);
      } catch (err) {
        logger.error('Failed to load messages', err);
        setError(err instanceof Error ? err.message : String(err));
      }
    };

    loadMessages();
  }, [currentSession?.id]);

  /**
   * Detect streaming completion and persist to React state
   * Implements idea.md architecture: "React owns the message stack"
   */
  useEffect(() => {
    if (!currentSession?.id) return;

    const streamingMsg = streamingMessages.get(currentSession.id);

    // Check if streaming just completed (isStreaming: false)
    if (streamingMsg && streamingMsg.isStreaming === false) {
      logger.debug('Streaming completed, persisting message', {
        sessionId: currentSession.id,
        messageId: streamingMsg.id,
      });

      // Add to messages if not already present
      setMessages((prev) => {
        const exists = prev.some((m) => m.id === streamingMsg.id);
        if (exists) return prev;
        return [...prev, streamingMsg as Message];
      });
    }
  }, [currentSession?.id, streamingMessages]);

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
          const sessionId = payload.session_id as string;

          // Only process events for current session
          if (sessionId !== currentSession.id) return;

          logger.debug('Received agent event', {
            sessionId,
            eventType,
          });

          // Handle different event types
          if (eventType === 'StatusChanged') {
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
          } else if (eventType === 'WorkflowError') {
            setWorkflowStatus('error');
            setIsLoading(false);
            setError((payload.error as string) ?? 'Unknown error');
          } else if (eventType === 'MessageAdded') {
            // ✅ Handle tool result messages from Rust
            const newMessage = payload.message as Message;

            logger.debug('MessageAdded event received', {
              sessionId: currentSession.id,
              messageId: newMessage.id,
              role: newMessage.role,
            });

            // Add to messages if not already present
            setMessages((prev) => {
              const exists = prev.some((m) => m.id === newMessage.id);
              if (exists) return prev;
              return [...prev, newMessage];
            });
          } else if (eventType === 'WorkflowCompleted') {
            // Workflow completed - messages already in React state per idea.md
            setWorkflowStatus('idle');
            setIsLoading(false);

            logger.info('Workflow completed', {
              sessionId: currentSession.id,
              messageCount: messages.length,
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
        setMessages((prev) => [...prev, message]);

        // Delegate to Rust backend (Rust will save the message to DB)
        await invoke('agent_send_message', {
          request: {
            sessionId: currentSession.id,
            message,
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
        setMessages((prev) => prev.filter((m) => m.id !== message.id));
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
    const lastUserMessage = [...messages]
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
    const messageIndex = messages.findIndex(
      (msg) => msg.id === lastUserMessage.id,
    );
    const messagesToDelete = messages.slice(messageIndex + 1);

    for (const msg of messagesToDelete) {
      await deleteMessage(msg.id);
    }

    // Re-submit the user message
    await submit(lastUserMessage);
  }, [currentSession?.id, messages, submit]);

  // Combine state values
  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isLoading,
      messages: displayMessages, // Use merged messages with streaming
      error,
      workflowStatus,
    }),
    [isLoading, displayMessages, error, workflowStatus],
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
