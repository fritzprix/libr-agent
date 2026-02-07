import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  useAgentSessionState,
  useAgentSessionActions,
} from './AgentSessionContext';
import { useLLMService } from './LLMServiceContext';
import { getLogger } from '../lib/logger';
import { isValidMessage } from '@/models/validation';
import type { Message, RustMessage } from '@/models/chat';

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
  error: string | null;
  llmError: string | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
  agentModeEnabled: boolean;
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
   * Toggle agent mode (autonomous tool use loop)
   */
  toggleAgentMode: () => void;

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

  const { streamingMessages, setAgentMode: setLLMAgentMode } = useLLMService();

  // Service contexts state (still local to Chat view as it's UI context)
  const [serviceContexts, setServiceContexts] = useState<
    Record<string, ServiceContext>
  >({});

  // Agent Mode state (local UI state, synced to LLMService)
  const [agentModeEnabled, setAgentModeEnabled] = useState(false);

  // Sync agent mode to LLMServiceContext when session or toggle changes
  useEffect(() => {
    if (session?.id) {
      setLLMAgentMode(session.id, agentModeEnabled);
    }
  }, [session?.id, agentModeEnabled, setLLMAgentMode]);

  // Reset agent mode when session changes
  useEffect(() => {
    setAgentModeEnabled(false);
  }, [session?.id]);

  // Fetch service contexts from backend
  const updateServiceContexts = useCallback(async () => {
    const sessionId = session?.id;
    if (!sessionId) return;

    try {
      const contexts = await invoke<Record<string, ServiceContext>>(
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
  useEffect(() => {
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
  }, [sessionMessages, updateServiceContexts]);

  /**
   * Extract streaming message for current session
   * Memoized to prevent unnecessary effect re-runs
   */
  const currentStreamingMessage = useMemo(() => {
    if (!session?.id) return undefined;
    return streamingMessages.get(session.id);
  }, [session?.id, streamingMessages]);

  /**
   * Merge persisted messages with streaming messages from LLMServiceContext
   * We use sessionMessages directly now, merging only the streaming tail.
   */
  const displayMessages = useMemo(() => {
    if (!session?.id) return [];

    // If there's a streaming message that's not yet in persisted messages
    if (
      isValidMessage(currentStreamingMessage) &&
      currentStreamingMessage.isStreaming !== false
    ) {
      const existsInMessages = sessionMessages.some(
        (m) => m.id === currentStreamingMessage.id,
      );
      if (!existsInMessages) {
        // Show streaming message alongside persisted messages
        return [...sessionMessages, currentStreamingMessage];
      }
    }

    // Return persisted messages only
    return sessionMessages;
  }, [sessionMessages, currentStreamingMessage, session?.id]);

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

      logger.info('Submitting message to agent workflow', {
        sessionId: session.id,
        messageId: message.id,
      });

      try {
        /*
         * Note: We don't need to manually update local state or isLoading here.
         * The 'agent:event' listener in AgentSessionContext will pick up 'statusChanged'
         * (busy) and 'messageAdded' events from the backend and update the shared state.
         *
         * However, for optimistic UI, we could technically append to messages in SessionContext
         * but sticking to "event driven" is cleaner.
         * To keep UI responsive, we rely on the backend sending events immediately.
         */

        // Convert Date objects to Unix timestamps for Rust backend
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

        // Delegate to Rust backend (Rust will save the message to DB)
        await invoke('agent_send_message', {
          request: {
            sessionId: session.id,
            message: messageForRust,
          },
        });

        addMessage(message);
      } catch (err) {
        logger.error('Failed to submit message', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        // Error state handled by AgentSessionContext via workflowError event or we could set it locally if needed,
        // but typically we let the global error handler work.
      }
    },
    [session?.id],
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

        await invoke('agent_inject_messages', {
          request: {
            sessionId: session.id,
            messages: messagesForRust,
            triggerWorkflow,
          },
        });
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

    try {
      await invoke('agent_terminate_workflow', {
        sessionId: session.id,
      });
      // Status update will come via event
    } catch (err) {
      logger.error('Failed to cancel workflow', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    }
  }, [session?.id]);

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

  /**
   * Toggle agent mode
   */
  const toggleAgentMode = useCallback(() => {
    setAgentModeEnabled((prev) => !prev);
    logger.info(`Agent mode ${!agentModeEnabled ? 'enabled' : 'disabled'}`);
  }, [agentModeEnabled]);

  // Combine state values
  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isSessionLoading,
      messages: displayMessages,
      error,
      llmError,
      workflowStatus,
      agentModeEnabled,
      serviceContexts,
    }),
    [
      isSessionLoading,
      displayMessages,
      error,
      llmError,
      workflowStatus,
      agentModeEnabled,
      serviceContexts,
    ],
  );

  // Combine action values
  const actionsValue: AgentChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel,
      retryMessage,
      toggleAgentMode,
      updateServiceContexts,
      injectMessages,
      resume: resumeSession,
    }),
    [
      submit,
      cancel,
      retryMessage,
      toggleAgentMode,
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
