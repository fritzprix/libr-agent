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
import { AIServiceProvider } from '@/lib/ai-service';
import { getLogger } from '../lib/logger';
import type { Message, RustMessage } from '@/models/chat';
import { deleteMessage } from '@/lib/backend/messages';
import { useSettings } from '@/hooks/use-settings';
import { supportsThinking } from '@/lib/ai-service/model-capabilities';

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
  reasoningEnabled: boolean;
  canUseReasoning: boolean;
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
   * Toggle reasoning mode (deep thinking for supported models)
   */
  toggleReasoning: () => void;

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

  const { setError, addMessage } = useAgentSessionActions();

  const { streamingMessages } = useLLMService();
  const { value: settingValue } = useSettings();

  // Service contexts state (still local to Chat view as it's UI context)
  const [serviceContexts, setServiceContexts] = useState<
    Record<string, ServiceContext>
  >({});

  const [reasoningEnabled, setReasoningEnabled] = useState(false);
  const [canUseReasoning, setCanUseReasoning] = useState(false);

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
   * Check if current model supports reasoning (matches ChatContext pattern)
   */
  useEffect(() => {
    const checkReasoningSupport = async () => {
      // Prioritize session-specific config, fallback to global settings
      const modelName =
        session?.assistant?.model || settingValue?.preferredModel?.model;
      const provider =
        session?.assistant?.provider || settingValue?.preferredModel?.provider;

      if (!modelName || !provider) {
        setCanUseReasoning(false);
        return;
      }

      try {
        const supports = await supportsThinking(
          modelName,
          provider as AIServiceProvider,
        );
        setCanUseReasoning(supports);

        // Auto-disable if model doesn't support reasoning
        if (!supports && reasoningEnabled) {
          setReasoningEnabled(false);
          logger.info('Reasoning disabled: model does not support it');
        }
      } catch (error) {
        logger.error('Failed to check reasoning support', error);
        setCanUseReasoning(false);
      }
    };

    checkReasoningSupport();
  }, [
    session?.assistant?.model,
    session?.assistant?.provider,
    settingValue?.preferredModel?.model,
    settingValue?.preferredModel?.provider,
    reasoningEnabled,
  ]);

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
      currentStreamingMessage?.id &&
      currentStreamingMessage.isStreaming !== false
    ) {
      const existsInMessages = sessionMessages.some(
        (m) => m.id === currentStreamingMessage.id,
      );
      if (!existsInMessages) {
        // Show streaming message alongside persisted messages
        return [...sessionMessages, currentStreamingMessage as Message];
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
   */
  const retryMessage = useCallback(async () => {
    if (!session?.id) {
      logger.error('Cannot retry: no active session');
      return;
    }

    // Find the last user message
    const lastUserMessage = [...sessionMessages]
      .reverse()
      .find((msg) => msg.role === 'user');

    if (!lastUserMessage) {
      logger.warn('No user message to retry');
      return;
    }

    logger.info('Retrying last message', {
      sessionId: session.id,
      messageId: lastUserMessage.id,
    });

    // Delete any subsequent messages (including failed assistant responses)
    const messageIndex = sessionMessages.findIndex(
      (msg) => msg.id === lastUserMessage.id,
    );
    const messagesToDelete = sessionMessages.slice(messageIndex + 1);

    for (const msg of messagesToDelete) {
      await deleteMessage(msg.id);
    }

    // We rely on backend/session context to refresh messages list via events or reload.
    // Ideally deleteMessage should probably trigger a reload or be handled by session actions.
    // For now, re-submitting will trigger new events.

    // Re-submit the user message
    await submit(lastUserMessage);
  }, [session?.id, sessionMessages, submit]);

  /**
   * Toggle reasoning mode
   */
  const toggleReasoning = useCallback(() => {
    if (!canUseReasoning) {
      logger.warn('Reasoning mode not supported for current model');
      return;
    }
    setReasoningEnabled((prev) => !prev);
    logger.info(`Reasoning mode ${!reasoningEnabled ? 'enabled' : 'disabled'}`);
  }, [canUseReasoning, reasoningEnabled]);

  // Combine state values
  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isSessionLoading,
      messages: displayMessages,
      error,
      llmError,
      workflowStatus,
      reasoningEnabled,
      canUseReasoning,
      serviceContexts,
    }),
    [
      isSessionLoading,
      displayMessages,
      error,
      llmError,
      workflowStatus,
      reasoningEnabled,
      canUseReasoning,
      serviceContexts,
    ],
  );

  // Combine action values
  const actionsValue: AgentChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel,
      retryMessage,
      toggleReasoning,
      updateServiceContexts,
      injectMessages,
    }),
    [
      submit,
      cancel,
      retryMessage,
      toggleReasoning,
      updateServiceContexts,
      injectMessages,
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
