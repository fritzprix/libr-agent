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
import type { Message, RustMessage } from '@/models/chat';
import { rustMessageToMessage } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { AgentSession } from '@/models/agent';

const logger = getLogger('AgentSessionContext');

type AgentEventPayload =
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
    };

// --- STATE CONTEXT ---
interface AgentSessionStateContextValue {
  session: AgentSession | null;
  messages: Message[];
  isSessionLoading: boolean;
  error: string | null;
  llmError: string | null;
  workflowStatus: 'idle' | 'busy' | 'paused' | 'error';
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

      try {
        // 1. Get session metadata
        const response = await invoke<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          createdAt: number;
          updatedAt?: number;
        } | null>('agent_get_session', {
          sessionId,
        });

        if (!response) {
          throw new Error(`Session not found: ${sessionId}`);
        }

        if (!isMounted) return;

        const sessionData: AgentSession = {
          id: response.id,
          name: response.name,
          status: response.status,
          createdAt: new Date(response.createdAt),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
        };

        setSession(sessionData);
        setWorkflowStatus(sessionData.status);

        // 2. Resume session in Rust backend (ensure active in memory)
        await invoke('agent_resume_session', { sessionId });

        // 3. Initialize session cache with messages in Rust
        await invoke('agent_init_session_with_messages', { sessionId });

        // 4. Load messages
        await loadMessages(sessionId);

        // 5. Setup Event Listener
        unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
          if (!isMounted) return;

          const payload = event.payload;

          // Strict Session Isolation: Only process events for THIS session
          if (payload.sessionId !== sessionId) {
            return;
          }

          logger.debug('Agent session event received', {
            type: payload.type,
            sessionId,
          });

          switch (payload.type) {
            case 'statusChanged': {
              const newStatus = payload.status;
              setWorkflowStatus(newStatus);
              setSession((prev) =>
                prev ? { ...prev, status: newStatus } : null,
              );
              break;
            }

            case 'workflowError': {
              setWorkflowStatus('error');
              setIsSessionLoading(false);
              const errorMsg = payload.error;

              if (
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

              setMessages((prev) => {
                if (prev.some((m) => m.id === newMessage.id)) return prev;
                return [...prev, newMessage];
              });
              break;
            }

            case 'workflowCompleted': {
              setWorkflowStatus('idle');
              setIsSessionLoading(false);
              break;
            }
          }
        });

        setIsSessionLoading(false);
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
    }),
    [session, messages, isSessionLoading, error, llmError, workflowStatus],
  );

  const actionsValue: AgentSessionActionsContextValue = useMemo(
    () => ({
      sendMessage,
      stopSession,
      addMessage,
      setError,
    }),
    [sendMessage, stopSession, addMessage, setError],
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
