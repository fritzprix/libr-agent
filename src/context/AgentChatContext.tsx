import { safeInvoke } from '@/lib/backend/core';
import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';

import { useDebounce } from 'react-use';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from './AgentSessionContext';
import { useLLMService } from './LLMServiceContext';
import { getLogger } from '../lib/logger';
import { isValidMessage } from '@/models/validation';
import type { Message, RustMessage } from '@/models/chat';
import type {
  SendUserMessageRequest,
  InjectMessagesRequest,
  AgentResponse,
} from '@/models/agent-ipc';

const logger = getLogger('AgentChatContext');

/**
 * Service Context from Rust backend
 */
export interface ServiceContext {
  contextPrompt: string;
  structuredState?: Record<string, unknown>;
}

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
  isSessionLoading: boolean;
  messages: Message[];
  pendingMessages: Message[]; // NEW: Export pending queue for set-based detection
  error: string | null;
  llmError: string | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
  serviceContexts: Record<string, ServiceContext>;
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

  /**
   * Manually update service contexts from backend
   */
  updateServiceContexts: () => Promise<void>;

  /**
   * Inject messages into the session directly
   * Optionally triggers the workflow based on the updated history
   */
  injectMessages: (
    messages: Message[],
    triggerWorkflow?: boolean,
  ) => Promise<void>;

  /**
   * Resume a paused workflow
   */
  resume: () => Promise<void>;
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
 * Simply delegates state from AgentSessionContext and actions to Rust backend.
 * Now purely reactive, with all message/status state residing in AgentSessionContext.
 */
export function AgentChatProvider({ children }: AgentChatProviderProps) {
  // Consume state from AgentSessionContext (Single Source of Truth)
  const {
    session,
    messages: sessionMessages, // Messages now come directly from session context
    isSessionLoading,
    workflowStatus,
    error,
    llmError,
  } = useAgentSessionState();

  const { setError, addMessage, resumeSession } = useAgentSessionActions();

  const { streamingMessages, cancelCompletionRequest } = useLLMService();

  // Service contexts state (still local to Chat view as it's UI context)
  const [serviceContexts, setServiceContexts] = useState<
    Record<string, ServiceContext>
  >({});

  // Pending messages queue for busy state
  const [pendingMessages, setPendingMessages] = useState<Message[]>([]);

  /**
   * Internal submit handler for merged messages
   * (Separated to avoid circular dependency in 'submit' which uses the queue)
   */
  const submitMergedMessage = useCallback(
    async (message: Message) => {
      if (!session?.id) return;

      logger.info('Submitting merged message', {
        sessionId: session.id,
        messageId: message.id,
      });

      try {
        const now = Date.now();
        const messageForRust: RustMessage = {
          ...message,
          toolCalls: message.tool_calls,
          toolCallId: message.tool_call_id,
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

        const request: SendUserMessageRequest = {
          sessionId: session.id,
          message: messageForRust,
        };

        await safeInvoke<AgentResponse>('agent_send_message', { request });

        addMessage(message);
      } catch (err) {
        logger.error('Failed to submit merged message', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [session?.id, addMessage, setError],
  );

  // Fetch service contexts from backend
  const updateServiceContexts = useCallback(async () => {
    const sessionId = session?.id;
    if (!sessionId) return;

    try {
      const contexts = await safeInvoke<Record<string, ServiceContext>>(
        'agent_get_service_contexts',
        { sessionId },
      );
      setServiceContexts(contexts);
      logger.info('Service contexts updated', {
        contexts,
      });
    } catch (error) {
      logger.error('Failed to update service contexts', error);
    }
  }, [session?.id]);

  // Initial fetch when session changes
  useEffect(() => {
    if (session?.id) {
      updateServiceContexts();
    } else {
      setServiceContexts({});
    }
  }, [session?.id, updateServiceContexts]);

  // Reactive Service Context Update:
  // When messages change, if the last message is from assistant, update service contexts.
  useDebounce(
    () => {
      if (sessionMessages.length > 0) {
        const lastMsg = sessionMessages[sessionMessages.length - 1];
        if (lastMsg.role === 'assistant') {
          updateServiceContexts().catch((err) =>
            logger.error(
              'Failed to update service contexts on message change',
              err,
            ),
          );
        }
      }
    },
    500,
    [sessionMessages, updateServiceContexts],
  );

  /**
   * Clean up pending messages when they appear in sessionMessages
   * This happens after Rust workflow processes them and emits MessageAdded events
   */
  useEffect(() => {
    if (pendingMessages.length === 0 || sessionMessages.length === 0) return;

    // Early exit: check if ANY pending message exists in sessionMessages
    const sessionMessageIds = new Set(sessionMessages.map((m) => m.id));
    const hasOverlap = pendingMessages.some((p) => sessionMessageIds.has(p.id));

    if (!hasOverlap) return; // No cleanup needed

    // Only log and process if we actually need to clean up
    logger.debug('Pending messages cleanup triggered', {
      pendingCount: pendingMessages.length,
      sessionMessagesCount: sessionMessages.length,
    });

    setPendingMessages((prev) => {
      const filtered = prev.filter(
        (pending) => !sessionMessageIds.has(pending.id),
      );

      if (filtered.length !== prev.length) {
        const removed = prev.filter((p) => sessionMessageIds.has(p.id));
        logger.info('Removed messages from pending queue', {
          removedCount: prev.length - filtered.length,
          removedIds: removed.map((p) => p.id),
        });
      }

      return filtered;
    });
  }, [sessionMessages, pendingMessages]);

  /**
   * Extract streaming message for current session
   * Memoized to prevent unnecessary effect re-runs
   */
  const currentStreamingMessage = useMemo(() => {
    if (!session?.id) return undefined;
    return streamingMessages.get(session.id);
  }, [session?.id, streamingMessages]);

  /**
   * Merge persisted messages with streaming messages AND pending messages
   */
  const displayMessages = useMemo(() => {
    if (!session?.id) return [];

    let displayed = [...sessionMessages];

    // Append pending messages (optimistic UI)
    if (pendingMessages.length > 0) {
      displayed = [...displayed, ...pendingMessages];
    }

    // If there's a streaming message that's not yet in persisted messages
    if (
      isValidMessage(currentStreamingMessage) &&
      currentStreamingMessage.isStreaming !== false
    ) {
      const existsInMessages = displayed.some(
        (m) => m.id === currentStreamingMessage.id,
      );
      if (!existsInMessages) {
        // Show streaming message alongside persisted messages
        displayed.push(currentStreamingMessage);
      }
    }

    return displayed;
  }, [sessionMessages, pendingMessages, currentStreamingMessage, session?.id]);

  /**
   * Submit a user message to the agent workflow
   * This delegates to Rust backend via agent_send_message command
   */
  const submit = useCallback(
    async (message: Message) => {
      if (!session?.id) {
        logger.error('Cannot submit: no active session');
        return;
      }

      // Check if we should inject directly to backend cache (for next recursion)
      if (
        workflowStatus === 'busy' ||
        workflowStatus === 'paused' ||
        isSessionLoading
      ) {
        logger.info('Agent busy/paused, injecting message to backend cache', {
          status: workflowStatus,
          messageId: message.id,
        });

        // Inject immediately into backend cache (no workflow trigger)
        // This makes the message available for the next LLM recursion
        // The workflow will pick this up after tool execution completes
        try {
          const now = Date.now();
          const messageForRust: RustMessage = {
            ...message,
            toolCalls: message.tool_calls,
            toolCallId: message.tool_call_id,
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

          const request: InjectMessagesRequest = {
            sessionId: session.id,
            messages: [messageForRust],
            triggerWorkflow: false, // Backend will include in next automatic LLM call
          };

          await safeInvoke<AgentResponse>('agent_inject_messages', { request });

          // Keep in pending queue for UI display purposes
          // Will be removed when backend processes and emits MessageAdded event
          setPendingMessages((prev) => {
            logger.info('Adding message to pending queue', {
              messageId: message.id,
              currentPendingCount: prev.length,
              newPendingCount: prev.length + 1,
            });
            return [...prev, message];
          });
        } catch (err) {
          logger.error('Failed to inject message to backend', err);
          const errorMessage = err instanceof Error ? err.message : String(err);
          setError(errorMessage);
          return;
        }

        return;
      }

      logger.info('Submitting message to agent workflow', {
        sessionId: session.id,
        messageId: message.id,
      });

      // Delegate to internal logic which handles error state and re-throws
      await submitMergedMessage(message);
    },
    [
      session?.id,
      workflowStatus,
      isSessionLoading,
      submitMergedMessage,
      setError,
    ],
  );

  /**
   * Inject messages into the session
   */
  const injectMessages = useCallback(
    async (messages: Message[], triggerWorkflow = false) => {
      if (!session?.id) {
        logger.error('Cannot inject messages: no active session');
        return;
      }

      logger.info('Injecting messages', {
        sessionId: session.id,
        count: messages.length,
        triggerWorkflow,
      });

      try {
        const now = Date.now();
        const messagesForRust: RustMessage[] = messages.map((msg) => ({
          ...msg,
          toolCalls: msg.tool_calls,
          toolCallId: msg.tool_call_id,
          createdAt:
            msg.createdAt instanceof Date
              ? msg.createdAt.getTime()
              : msg.createdAt || now,
          updatedAt:
            msg.updatedAt instanceof Date
              ? msg.updatedAt.getTime()
              : msg.updatedAt ||
                (msg.createdAt instanceof Date
                  ? msg.createdAt.getTime()
                  : msg.createdAt) ||
                now,
        }));

        const request: InjectMessagesRequest = {
          sessionId: session.id,
          messages: messagesForRust,
          triggerWorkflow,
        };

        await safeInvoke<AgentResponse>('agent_inject_messages', { request });
        // Events will update the UI
      } catch (err) {
        logger.error('Failed to inject messages', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
      }
    },
    [session?.id],
  );

  /**
   * Cancel the current workflow
   */
  const cancel = useCallback(async () => {
    if (!session?.id) {
      logger.error('Cannot cancel: no active session');
      return;
    }

    logger.info('Cancelling workflow', { sessionId: session.id });

    // 1. Immediately cancel any local streaming LLM requests
    cancelCompletionRequest(session.id);

    // 2. Clear pending messages to remove them optimistically from UI
    setPendingMessages([]);

    // 3. Inform Rust backend to cancel the workflow loop
    try {
      await safeInvoke<AgentResponse>('agent_cancel_workflow', {
        sessionId: session.id,
      });
      // Status update will come via event
    } catch (err) {
      logger.error('Failed to cancel workflow', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    }
  }, [session?.id, cancelCompletionRequest, setError]);

  /**
   * Retry the last failed message
   *
   * Error state uses the same recovery mechanism as Paused:
   * - No message deletion
   * - Resume from last saved state with current message stack
   * - Only difference from Paused is the UI display (error message)
   */
  const retryMessage = useCallback(async () => {
    if (!session?.id) {
      logger.error('Cannot retry: no active session');
      return;
    }

    logger.info('Retrying workflow after error', {
      sessionId: session.id,
    });

    // Use the same mechanism as resume (Paused state)
    // This preserves the complete message stack
    try {
      await resumeSession();
    } catch (err) {
      logger.error('Failed to retry workflow', err);
      throw err;
    }
  }, [session?.id, resumeSession]);

  // Combine state values
  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isSessionLoading,
      messages: displayMessages,
      pendingMessages, // NEW: Expose pending queue for set-based detection
      error,
      llmError,
      workflowStatus,
      serviceContexts,
    }),
    [
      isSessionLoading,
      displayMessages,
      pendingMessages, // NEW: Add to dependencies
      error,
      llmError,
      workflowStatus,
      serviceContexts,
    ],
  );

  // Combine action values
  const actionsValue: AgentChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel,
      retryMessage,
      updateServiceContexts,
      injectMessages,
      resume: resumeSession,
    }),
    [
      submit,
      cancel,
      retryMessage,
      updateServiceContexts,
      injectMessages,
      resumeSession,
    ],
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
