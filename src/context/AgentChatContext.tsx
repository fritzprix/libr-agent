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

import {
  useAgentSessionActions,
  useAgentSessionState,
} from '@/context/AgentSessionContext';
import type { AgentResponse, InjectMessagesRequest } from '@/models/agent-ipc';
import type { Message, MessageError, RustMessage } from '@/models/chat';
import type { ServiceContext } from '@/models/service-context';
import { isValidMessage } from '@/models/validation';
import {
  isAssistantStreamingMessageSuperseded,
  summarizeMessageForLog,
  toRustMessage,
} from '@/lib/message-utils';
import { useDebounce } from 'react-use';
import { getLogger } from '../lib/logger';
import { useLLMService, useStreamingMessage } from './LLMServiceContext';

const logger = getLogger('AgentChatContext');

/**
 * ⚡ Bolt: Optimized helper to find the last persisted assistant message without O(N) array clone and reverse
 */
const findLastPersistedAssistantMessage = (messages: Message[]): Message | undefined => {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === 'assistant' && !messages[i].isStreaming) {
      return messages[i];
    }
  }
  return undefined;
};

export { isAssistantStreamingMessageSuperseded } from '@/lib/message-utils';

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
  error: MessageError | null;
  llmError: MessageError | null;
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
   * Backend decides whether the workflow should continue based on session state
   */
  injectMessages: (messages: Message[]) => Promise<void>;

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

  const { setError, resumeSession } = useAgentSessionActions();

  const { cancelCompletionRequest, clearStreamingMessage } = useLLMService();

  // Service contexts state (still local to Chat view as it's UI context)
  const [serviceContexts, setServiceContexts] = useState<
    Record<string, ServiceContext>
  >({});

  // Pending messages queue for busy state
  const [pendingMessages, setPendingMessages] = useState<Message[]>([]);
  const previousSessionIdRef = useRef<string | null>(session?.id ?? null);
  const activeSessionIdRef = useRef<string | null>(session?.id ?? null);

  useEffect(() => {
    const nextSessionId = session?.id ?? null;
    activeSessionIdRef.current = nextSessionId;

    if (previousSessionIdRef.current === nextSessionId) {
      return;
    }

    previousSessionIdRef.current = nextSessionId;
    setPendingMessages([]);
    setServiceContexts({});
  }, [session?.id]);

  const enqueuePendingMessage = useCallback((message: Message) => {
    setPendingMessages((prev) => {
      if (prev.some((pending) => pending.id === message.id)) {
        return prev;
      }
      return [...prev, message];
    });
  }, []);

  const removePendingMessage = useCallback((messageId: string) => {
    setPendingMessages((prev) => {
      return prev.filter((message) => message.id !== messageId);
    });
  }, []);

  // Fetch service contexts from backend
  const updateServiceContexts = useCallback(async () => {
    const sessionId = session?.id;
    if (!sessionId) return;

    try {
      const contexts = await safeInvoke<Record<string, ServiceContext>>(
        'agent_get_service_contexts',
        { sessionId },
      );
      if (activeSessionIdRef.current !== sessionId) {
        return;
      }
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
      const removed = prev.filter((pending) =>
        sessionMessageIds.has(pending.id),
      );
      const filtered = prev.filter(
        (pending) => !sessionMessageIds.has(pending.id),
      );

      if (filtered.length !== prev.length) {
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
  const currentStreamingMessage = useStreamingMessage(session?.id);

  useEffect(() => {
    if (!session?.id || !isValidMessage(currentStreamingMessage)) {
      return;
    }

    const lastPersistedAssistantMessage = findLastPersistedAssistantMessage(sessionMessages);

    if (!lastPersistedAssistantMessage) {
      return;
    }

    if (
      isAssistantStreamingMessageSuperseded(
        currentStreamingMessage,
        lastPersistedAssistantMessage,
      )
    ) {
      logger.info('Clearing superseded streaming assistant message', {
        sessionId: session.id,
        streaming: summarizeMessageForLog(currentStreamingMessage),
        persisted: summarizeMessageForLog(lastPersistedAssistantMessage),
      });
      clearStreamingMessage(session.id);
    }
  }, [
    clearStreamingMessage,
    currentStreamingMessage,
    session?.id,
    sessionMessages,
  ]);

  /**
   * Merge persisted messages with streaming messages AND pending messages
   */
  const displayMessages = useMemo(() => {
    if (!session?.id) return [];

    const displayed = [...sessionMessages];
    const displayedIds = new Set(displayed.map((message) => message.id));
    const lastPersistedAssistantMessage = findLastPersistedAssistantMessage(sessionMessages);

    // Append pending messages (optimistic UI)
    if (pendingMessages.length > 0) {
      pendingMessages.forEach((message) => {
        if (displayedIds.has(message.id)) {
          return;
        }

        displayed.push(message);
        displayedIds.add(message.id);
      });
    }

    // If there's a streaming message that's not yet in persisted messages
    if (isValidMessage(currentStreamingMessage)) {
      const isSupersededByPersistedAssistant =
        !!lastPersistedAssistantMessage &&
        isAssistantStreamingMessageSuperseded(
          currentStreamingMessage,
          lastPersistedAssistantMessage,
        );

      if (
        currentStreamingMessage.tool_calls &&
        currentStreamingMessage.tool_calls.length > 0
      ) {
        logger.info('Evaluating streaming message for display', {
          sessionId: session.id,
          streaming: summarizeMessageForLog(currentStreamingMessage),
          lastPersistedAssistant: summarizeMessageForLog(
            lastPersistedAssistantMessage,
          ),
          isSupersededByPersistedAssistant,
          displayedIds: [...displayedIds],
        });
      }

      if (
        !displayedIds.has(currentStreamingMessage.id) &&
        !isSupersededByPersistedAssistant
      ) {
        // Show streaming message alongside persisted messages
        displayed.push(currentStreamingMessage);
        if (
          currentStreamingMessage.tool_calls &&
          currentStreamingMessage.tool_calls.length > 0
        ) {
          logger.info('Streaming message appended to displayMessages', {
            sessionId: session.id,
            streamingMessageId: currentStreamingMessage.id,
            displayCount: displayed.length,
          });
        }
      }
    }

    return displayed;
  }, [sessionMessages, pendingMessages, currentStreamingMessage, session?.id]);

  /**
   * Inject messages into the session
   */
  const injectMessages = useCallback(
    async (messages: Message[]) => {
      if (!session?.id) {
        logger.error('Cannot inject messages: no active session');
        return;
      }

      logger.info('Injecting messages', {
        sessionId: session.id,
        count: messages.length,
        status: workflowStatus,
      });

      try {
        const messagesForRust: RustMessage[] = messages.map(toRustMessage);

        const request: InjectMessagesRequest = {
          sessionId: session.id,
          messages: messagesForRust,
        };

        await safeInvoke<AgentResponse>('agent_inject_messages', { request });
        // Events will update the UI
      } catch (err) {
        logger.error('Failed to inject messages', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [session?.id, setError, workflowStatus],
  );

  /**
   * Submit a user message to the agent workflow
   */
  const submit = useCallback(
    async (message: Message) => {
      if (!session?.id) {
        logger.error('Cannot submit: no active session');
        return;
      }

      if (isSessionLoading) {
        const errorMessage = 'Cannot submit while session is still loading';
        logger.warn(errorMessage, {
          sessionId: session.id,
          messageId: message.id,
        });
        setError(errorMessage);
        throw new Error(errorMessage);
      }

      logger.info('Submitting message through injection path', {
        sessionId: session.id,
        messageId: message.id,
        status: workflowStatus,
      });

      enqueuePendingMessage(message);

      try {
        await injectMessages([message]);
      } catch (err) {
        removePendingMessage(message.id);
        throw err;
      }
    },
    [
      enqueuePendingMessage,
      injectMessages,
      isSessionLoading,
      removePendingMessage,
      session?.id,
      setError,
      workflowStatus,
    ],
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
    clearStreamingMessage(session.id);

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
  }, [session?.id, cancelCompletionRequest, clearStreamingMessage, setError]);

  /**
   * Retry the last failed message
   *
   * Error-state retry reuses the paused-session resume path:
   * - Frontend calls `resumeSession()`, which invokes `agent_resume_workflow`
   * - Rust rebuilds the request from the current stack, merging user turns and
   *   dropping incomplete tool chains before reevaluating preflight compaction
   * - No persisted messages are deleted; recovery happens by replaying from the
   *   sanitized in-memory/database context
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
