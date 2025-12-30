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
import type { Assistant, Message } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { useModelOptions } from './ModelProvider';

const logger = getLogger('AgentSessionContext');

/**
 * Agent session metadata from Rust backend
 */
interface AgentSession {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  createdAt: Date;
  updatedAt?: Date;
}

/**
 * Agent configuration for creating a new session
 */
interface CreateSessionParams {
  assistant: Assistant;
  name?: string;
  // LLM config will be extracted from assistant or global settings
}

interface AgentEventPayload {
  type: string;
  sessionId: string;
  status?: 'idle' | 'busy' | 'paused' | 'error';
  [key: string]: unknown;
}

// --- STATE CONTEXT ---
interface AgentSessionStateContextValue {
  currentSession: AgentSession | null;
  sessions: AgentSession[];
  messages: Message[];
  isLoading: boolean;
  isLoadingSessions: boolean;
  error: string | null;
}

const AgentSessionStateContext = createContext<
  AgentSessionStateContextValue | undefined
>(undefined);

// --- ACTIONS CONTEXT ---
interface AgentSessionActionsContextValue {
  /**
   * Create a new agent session
   */
  createSession: (params: CreateSessionParams) => Promise<AgentSession>;

  /**
   * Resume an existing agent session
   */
  resumeSession: (sessionId: string) => Promise<AgentSession>;

  /**
   * Send a user message to the current session
   */
  sendMessage: (content: string) => Promise<void>;

  /**
   * Stop the current session workflow
   */
  stopSession: () => Promise<void>;

  /**
   * Clear current session
   */
  clearSession: () => void;

  /**
   * Load all agent sessions
   */
  loadSessions: () => Promise<void>;

  /**
   * Delete an agent session
   */
  deleteSession: (sessionId: string) => Promise<void>;
}

const AgentSessionActionsContext = createContext<
  AgentSessionActionsContextValue | undefined
>(undefined);

interface AgentSessionProviderProps {
  children: React.ReactNode;
}

/**
 * AgentSessionProvider
 *
 * Manages agent session lifecycle (create, resume, terminate).
 * This is separate from SessionContext (V1/IndexedDB) and handles V2 Rust backend sessions.
 *
 * Reference: idea.md - "Chat 시작" and "Resume Chat History" scenarios
 */
export function AgentSessionProvider({ children }: AgentSessionProviderProps) {
  const { modelId, provider } = useModelOptions();
  const [currentSession, setCurrentSession] = useState<AgentSession | null>(
    null,
  );
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingSessions, setIsLoadingSessions] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /**
   * Load messages for the current session
   */
  const loadMessages = useCallback(async (sessionId: string) => {
    try {
      // Load first page (large size to get all for now)
      const page = await invoke<Page<Record<string, unknown>>>(
        'messages_get_page',
        {
          sessionId,
          page: 1,
          pageSize: 1000,
        },
      );

      // Deserialize messages from Rust backend format to frontend format
      const deserializedMessages: Message[] = page.items.map((rustMsg) => ({
        id: rustMsg.id as string,
        sessionId: rustMsg.sessionId as string,
        threadId: (rustMsg.threadId as string) || (rustMsg.sessionId as string), // Fallback to sessionId
        role: rustMsg.role as 'user' | 'assistant' | 'system' | 'tool',
        content: rustMsg.content as Message['content'],
        tool_calls: (rustMsg.toolCalls as Message['tool_calls'])
          ? (rustMsg.toolCalls as Message['tool_calls'])?.map((tc) => ({
              id: tc.id,
              type: tc.type || 'function',
              function: tc.function,
            }))
          : undefined,
        tool_call_id: rustMsg.toolCallId as string | undefined,
        isStreaming: rustMsg.isStreaming as boolean | undefined,
        thinking: rustMsg.thinking as string | undefined,
        thinkingSignature: rustMsg.thinkingSignature as string | undefined,
        assistantId: rustMsg.assistantId as string | undefined,
        attachments:
          (rustMsg.attachments as Message['attachments']) || undefined,
        tool_use: (rustMsg.toolUse as Message['tool_use']) || undefined,
        createdAt: rustMsg.createdAt
          ? new Date(rustMsg.createdAt as number)
          : new Date(),
        updatedAt: rustMsg.updatedAt
          ? new Date(rustMsg.updatedAt as number)
          : undefined,
        source: rustMsg.source as 'assistant' | 'ui' | undefined,
        error: (rustMsg.error as Message['error']) || undefined,
      }));

      setMessages(deserializedMessages);
    } catch (err) {
      logger.error('Failed to load messages', err);
    }
  }, []);

  /**
   * Listen for agent events
   */
  useEffect(() => {
    if (!currentSession) return;

    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
        const payload = event.payload;

        // Filter events for current session
        if (payload.sessionId !== currentSession.id) return;

        logger.debug('Received agent event', payload);

        switch (payload.type) {
          case 'statusChanged':
            if (payload.status) {
              setCurrentSession((prev) =>
                prev
                  ? {
                      ...prev,
                      status: payload.status as AgentSession['status'],
                    }
                  : null,
              );
            }
            break;
          case 'messageAdded':
            // Reload messages to get the new one
            // Optimization: could just fetch the single message if ID provided
            loadMessages(currentSession.id);
            break;
          case 'workflowStarted':
          case 'workflowCompleted':
          case 'workflowError':
            // These might imply status changes or other UI updates
            break;
        }
      });
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
    };
  }, [currentSession?.id, loadMessages]);

  /**
   * Create a new agent session
   * Corresponds to: StartChatView -> useAgentSession -> AgentSessionManager.createSession
   */
  const createSession = useCallback(
    async (params: CreateSessionParams): Promise<AgentSession> => {
      const { assistant, name } = params;

      logger.info('Creating new agent session', {
        assistantName: assistant.name,
        sessionName: name,
      });

      setIsLoading(true);
      setError(null);

      try {
        // Build agent config from assistant
        const agentConfig = {
          id: assistant.id,
          name: assistant.name,
          description: assistant.description,
          systemPrompt: assistant.systemPrompt,
          mcpServerIds: assistant.mcpServerIds || [],
          localServices: assistant.localServices || [],
          allowedBuiltInServiceAliases: assistant.allowedBuiltInServiceAliases,
          // Use selected model from ModelProvider
          model: modelId,
          provider: provider,
          temperature: 1.0,
          maxTokens: 8192,
        };

        // Generate session ID
        const { createId } = await import('@paralleldrive/cuid2');
        const sessionId = createId();

        // Call Rust backend to create session
        const response = await invoke<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          createdAt: number;
          updatedAt?: number;
        }>('agent_create_session', {
          request: {
            sessionId,
            name: name || `Conversation with ${assistant.name}`,
            agentConfig,
          },
        });

        const respWithDates = response as {
          createdAt?: number;
          created_at?: number;
        };
        const createdAtMs = respWithDates.createdAt ?? respWithDates.created_at;

        const session: AgentSession = {
          id: response.id,
          name: response.name,
          status:
            (response.status as 'idle' | 'busy' | 'paused' | 'error') || 'idle',
          createdAt: createdAtMs ? new Date(createdAtMs) : new Date(),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
        };

        setCurrentSession(session);
        setMessages([]); // New session has no messages
        setIsLoading(false);

        logger.info('Agent session created successfully', {
          sessionId: session.id,
        });

        return session;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        logger.error('Failed to create agent session', err);
        setError(errorMessage);
        setIsLoading(false);
        throw err;
      }
    },
    [modelId, provider],
  );

  /**
   * Resume an existing agent session
   * Corresponds to: SessionHistory -> useAgentSession -> AgentSessionManager.resumeSession
   */
  const resumeSession = useCallback(
    async (sessionId: string): Promise<AgentSession> => {
      logger.info('Resuming agent session', { sessionId });

      setIsLoading(true);
      setError(null);

      try {
        // Call Rust backend to get session metadata
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

        const respWithDates = response as {
          createdAt?: number;
          created_at?: number;
        };
        const createdAtMs = respWithDates.createdAt ?? respWithDates.created_at;

        const session: AgentSession = {
          id: response.id,
          name: response.name,
          status: response.status as 'idle' | 'busy' | 'paused' | 'error',
          createdAt: createdAtMs ? new Date(createdAtMs) : new Date(),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
        };

        setCurrentSession(session);
        await loadMessages(sessionId); // Load messages
        setIsLoading(false);

        logger.info('Agent session resumed successfully', {
          sessionId: session.id,
        });

        return session;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : String(err);
        logger.error('Failed to resume agent session', err);
        setError(errorMessage);
        setIsLoading(false);
        throw err;
      }
    },
    [loadMessages],
  );

  /**
   * Send a user message to the current session
   */
  const sendMessage = useCallback(
    async (content: string) => {
      if (!currentSession) {
        throw new Error('No active session');
      }

      try {
        const messageId = `msg_${Date.now()}`; // Temporary ID, backend might generate one
        const now = new Date();
        const message: Message = {
          id: messageId,
          sessionId: currentSession.id,
          threadId: currentSession.id,
          role: 'user',
          content: [{ type: 'text', text: content }],
          createdAt: now,
          updatedAt: now,
        };

        await invoke('agent_send_message', {
          request: {
            sessionId: currentSession.id,
            message,
          },
        });

        // Optimistic update? Or wait for event?
        // Waiting for event is safer for consistency
      } catch (err) {
        logger.error('Failed to send message', err);
        throw err;
      }
    },
    [currentSession],
  );

  /**
   * Stop the current session workflow
   */
  const stopSession = useCallback(async () => {
    if (!currentSession) return;

    try {
      await invoke('agent_terminate_workflow', {
        sessionId: currentSession.id,
      });
    } catch (err) {
      logger.error('Failed to stop session', err);
      throw err;
    }
  }, [currentSession]);

  /**
   * Clear current session (UI-only, does not terminate backend session)
   */
  const clearSession = useCallback(() => {
    logger.info('Clearing current agent session');
    setCurrentSession(null);
    setMessages([]);
    setError(null);
  }, []);

  /**
   * Load all agent sessions
   */
  const loadSessions = useCallback(async () => {
    logger.info('Loading all agent sessions');
    setIsLoadingSessions(true);

    try {
      // Call Rust backend to get all sessions
      const response = await invoke<
        Array<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          createdAt: number;
          updatedAt?: number;
        }>
      >('agent_get_all_sessions');

      const sessionList: AgentSession[] = response.map((s) => {
        return {
          id: s.id,
          name: s.name,
          status: s.status,
          createdAt: new Date(s.createdAt),
          updatedAt: s.updatedAt ? new Date(s.updatedAt) : undefined,
        };
      });

      setSessions(sessionList);
      logger.info('Loaded sessions', { count: sessionList.length });
    } catch (err) {
      logger.error('Failed to load sessions', err);
      setSessions([]);
    } finally {
      setIsLoadingSessions(false);
    }
  }, []);

  /**
   * Delete an agent session
   */
  const deleteSession = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting agent session', { sessionId });

      try {
        await invoke('agent_delete_session', { sessionId });

        // Remove from sessions list
        setSessions((prev) => prev.filter((s) => s.id !== sessionId));

        // Clear current session if it's the one being deleted
        if (currentSession?.id === sessionId) {
          clearSession();
        }

        logger.info('Session deleted successfully', { sessionId });
      } catch (err) {
        logger.error('Failed to delete session', err);
        throw err;
      }
    },
    [currentSession?.id, clearSession],
  );

  // Combine state values
  const stateValue: AgentSessionStateContextValue = useMemo(
    () => ({
      currentSession,
      sessions,
      messages,
      isLoading,
      isLoadingSessions,
      error,
    }),
    [currentSession, sessions, messages, isLoading, isLoadingSessions, error],
  );

  // Combine action values
  const actionsValue: AgentSessionActionsContextValue = useMemo(
    () => ({
      createSession,
      resumeSession,
      sendMessage,
      stopSession,
      clearSession,
      loadSessions,
      deleteSession,
    }),
    [
      createSession,
      resumeSession,
      sendMessage,
      stopSession,
      clearSession,
      loadSessions,
      deleteSession,
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
 * This matches the useAgentSession mentioned in idea.md
 */
export function useAgentSession() {
  const state = useAgentSessionState();
  const actions = useAgentSessionActions();

  return {
    ...state,
    ...actions,
  };
}
