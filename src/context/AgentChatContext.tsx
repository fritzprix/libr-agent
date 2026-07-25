import { safeInvoke } from '@/lib/backend/core';
import {
  cancelAgentPendingPrompt,
  getAgentPendingQueue,
} from '@/lib/backend/agent-commands';
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

import {
  useAgentSessionActions,
  useAgentSessionState,
} from '@/context/AgentSessionContext';
import type { AgentEventPayload } from '@/context/agent-session/types';
import type { AgentResponse, InjectMessagesRequest } from '@/models/agent-ipc';
import type { Message, MessageError, RustMessage } from '@/models/chat';
import { rustMessageToMessage } from '@/models/chat';
import type { ServiceContext } from '@/models/service-context';
import { isValidMessage } from '@/models/validation';
import { isAssistantStreamingMessageSuperseded } from '@/lib/message-streaming-supersession';
import { summarizeMessageForLog, toRustMessage } from '@/lib/message-utils';
import { isInactiveWorkflowStatus } from '@/context/agent-session/workflow-inactive-cleanup';
import { useDebounce } from 'react-use';
import { getLogger } from '../lib/logger';
import { useLLMService, useStreamingMessage } from './LLMServiceContext';

const logger = getLogger('AgentChatContext');

/**
 * ⚡ Bolt: Optimized helper to find the last persisted assistant message without O(N) array clone and reverse
 */
const findLastPersistedAssistantMessage = (
  messages: Message[],
): Message | undefined => {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === 'assistant' && !messages[i].isStreaming) {
      return messages[i];
    }
  }
  return undefined;
};

export { isAssistantStreamingMessageSuperseded } from '@/lib/message-streaming-supersession';

// --- STATE CONTEXT ---
interface AgentChatStateContextValue {
  isSessionLoading: boolean;
  messages: Message[];
  /** Waiting prompts (FIFO) shown above the input — not in the message list. */
  pendingQueue: Message[];
  error: MessageError | null;
  llmError: MessageError | null;
  workflowStatus:
    | 'idle'
    | 'busy'
    | 'paused'
    | 'error'
    | 'queued'
    | 'provisioning';
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
   * Cancel a single waiting prompt without aborting the active turn.
   */
  cancelPendingPrompt: (messageId: string) => Promise<void>;

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

function mapPendingQueue(messages: RustMessage[]): Message[] {
  return messages.map(rustMessageToMessage);
}

/**
 * AgentChatProvider
 *
 * Simply delegates state from AgentSessionContext and actions to Rust backend.
 * Now purely reactive, with all message/status state residing in AgentSessionContext.
 */
export function AgentChatProvider({ children }: AgentChatProviderProps) {
  const {
    session,
    messages: sessionMessages,
    isSessionLoading,
    workflowStatus,
    error,
    llmError,
  } = useAgentSessionState();

  const { setError, resumeSession } = useAgentSessionActions();

  const { cancelCompletionRequest, clearStreamingMessage } = useLLMService();

  const [serviceContexts, setServiceContexts] = useState<
    Record<string, ServiceContext>
  >({});

  const [pendingQueue, setPendingQueue] = useState<Message[]>([]);
  const [prevSessionId, setPrevSessionId] = useState<string | null>(
    session?.id ?? null,
  );
  const activeSessionIdRef = useRef<string | null>(session?.id ?? null);

  const nextSessionId = session?.id ?? null;

  if (nextSessionId !== prevSessionId) {
    setPrevSessionId(nextSessionId);
    setPendingQueue([]);
    setServiceContexts({});
  }

  useEffect(() => {
    activeSessionIdRef.current = nextSessionId;
  }, [nextSessionId]);

  const loadPendingQueue = useCallback(async (sessionId: string) => {
    try {
      const items = await getAgentPendingQueue(sessionId);
      if (activeSessionIdRef.current !== sessionId) {
        return;
      }
      setPendingQueue(mapPendingQueue(items));
    } catch (err) {
      logger.error('Failed to load pending queue', err);
    }
  }, []);

  useEffect(() => {
    if (!session?.id) {
      setPendingQueue([]);
      return;
    }
    void loadPendingQueue(session.id);
  }, [session?.id, loadPendingQueue]);

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

  useEffect(() => {
    if (session?.id) {
      updateServiceContexts();
    } else {
      setServiceContexts({});
    }
  }, [session?.id, updateServiceContexts]);

  useEffect(() => {
    const sessionId = session?.id;
    if (!sessionId) {
      return;
    }

    let unlisten: (() => void) | undefined;
    let isMounted = true;

    void listen<AgentEventPayload>('agent:event', (event) => {
      if (!isMounted) return;

      const payload = event.payload;
      if (payload.type === 'pendingQueueUpdated') {
        if (payload.sessionId !== sessionId) {
          return;
        }
        setPendingQueue(mapPendingQueue(payload.messages));
        return;
      }

      if (payload.type !== 'resourceUpdated' || payload.action !== 'clear') {
        return;
      }
      if (payload.resourceId !== sessionId) {
        return;
      }

      if (payload.resourceType === 'session') {
        setServiceContexts({});
        setPendingQueue([]);
        void updateServiceContexts().catch((err: unknown) =>
          logger.error(
            'Failed to refresh service contexts after session clear',
            err,
          ),
        );
        return;
      }

      if (payload.resourceType === 'planning') {
        void updateServiceContexts().catch((err: unknown) =>
          logger.error(
            'Failed to refresh service contexts after planning clear',
            err,
          ),
        );
      }
    }).then((fn) => {
      if (isMounted) {
        unlisten = fn;
      } else {
        fn();
      }
    });

    return () => {
      isMounted = false;
      if (unlisten) unlisten();
    };
  }, [session?.id, updateServiceContexts]);

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

  const currentStreamingMessage = useStreamingMessage(session?.id);

  useEffect(() => {
    if (!session?.id || !isValidMessage(currentStreamingMessage)) {
      return;
    }

    const lastPersistedAssistantMessage =
      findLastPersistedAssistantMessage(sessionMessages);

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

  const displayMessages = useMemo(() => {
    if (!session?.id) return [];

    const displayed = [...sessionMessages];
    const displayedIds = new Set(displayed.map((message) => message.id));
    const lastPersistedAssistantMessage =
      findLastPersistedAssistantMessage(sessionMessages);

    if (
      !isInactiveWorkflowStatus(workflowStatus) &&
      isValidMessage(currentStreamingMessage)
    ) {
      const isSupersededByPersistedAssistant =
        !!lastPersistedAssistantMessage &&
        isAssistantStreamingMessageSuperseded(
          currentStreamingMessage,
          lastPersistedAssistantMessage,
        );

      if (
        !displayedIds.has(currentStreamingMessage.id) &&
        !isSupersededByPersistedAssistant
      ) {
        displayed.push(currentStreamingMessage);
      }
    }

    return displayed;
  }, [sessionMessages, currentStreamingMessage, session?.id, workflowStatus]);

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
      } catch (err) {
        logger.error('Failed to inject messages', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [session?.id, setError, workflowStatus],
  );

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

      await injectMessages([message]);
    },
    [injectMessages, isSessionLoading, session?.id, setError, workflowStatus],
  );

  const cancel = useCallback(async () => {
    if (!session?.id) {
      logger.error('Cannot cancel: no active session');
      return;
    }

    logger.info('Cancelling workflow', { sessionId: session.id });

    cancelCompletionRequest(session.id);
    clearStreamingMessage(session.id);

    try {
      await safeInvoke<AgentResponse>('agent_cancel_workflow', {
        sessionId: session.id,
      });
    } catch (err) {
      logger.error('Failed to cancel workflow', err);
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
    }
  }, [session?.id, cancelCompletionRequest, clearStreamingMessage, setError]);

  const cancelPendingPrompt = useCallback(
    async (messageId: string) => {
      if (!session?.id) {
        logger.error('Cannot cancel pending prompt: no active session');
        return;
      }

      try {
        await cancelAgentPendingPrompt(session.id, messageId);
      } catch (err) {
        logger.error('Failed to cancel pending prompt', err);
        const errorMessage = err instanceof Error ? err.message : String(err);
        setError(errorMessage);
        throw err;
      }
    },
    [session?.id, setError],
  );

  const retryMessage = useCallback(async () => {
    if (!session?.id) {
      logger.error('Cannot retry: no active session');
      return;
    }

    logger.info('Retrying workflow after error', {
      sessionId: session.id,
    });

    try {
      await resumeSession();
    } catch (err) {
      logger.error('Failed to retry workflow', err);
      throw err;
    }
  }, [session?.id, resumeSession]);

  const stateValue: AgentChatStateContextValue = useMemo(
    () => ({
      isSessionLoading,
      messages: displayMessages,
      pendingQueue,
      error,
      llmError,
      workflowStatus,
      serviceContexts,
    }),
    [
      isSessionLoading,
      displayMessages,
      pendingQueue,
      error,
      llmError,
      workflowStatus,
      serviceContexts,
    ],
  );

  const actionsValue: AgentChatActionsContextValue = useMemo(
    () => ({
      submit,
      cancel,
      cancelPendingPrompt,
      retryMessage,
      updateServiceContexts,
      injectMessages,
      resume: resumeSession,
    }),
    [
      submit,
      cancel,
      cancelPendingPrompt,
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

export function useAgentChatState(): AgentChatStateContextValue {
  const context = useContext(AgentChatStateContext);
  if (!context) {
    throw new Error('useAgentChatState must be used within AgentChatProvider');
  }
  return context;
}

export function useAgentChatActions(): AgentChatActionsContextValue {
  const context = useContext(AgentChatActionsContext);
  if (!context) {
    throw new Error(
      'useAgentChatActions must be used within AgentChatProvider',
    );
  }
  return context;
}

export function useAgentChat() {
  const state = useAgentChatState();
  const actions = useAgentChatActions();

  return {
    ...state,
    ...actions,
  };
}
