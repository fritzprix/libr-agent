import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '../lib/logger';
import { useModelOptions } from './ModelProvider';
import { AgentSession, CreateSessionParams } from '@/models/agent';
import type { Assistant } from '@/models/chat';

const logger = getLogger('AgentSessionListContext');

// --- STATE CONTEXT ---
interface AgentSessionListStateContextValue {
  sessions: AgentSession[];
  isSessionsListLoading: boolean;
}

const AgentSessionListStateContext = createContext<
  AgentSessionListStateContextValue | undefined
>(undefined);

// --- ACTIONS CONTEXT ---
interface AgentSessionListActionsContextValue {
  /**
   * Create a new agent session
   */
  createSession: (params: CreateSessionParams) => Promise<AgentSession>;

  /**
   * Load all agent sessions
   */
  loadSessions: () => Promise<void>;

  /**
   * Delete an agent session
   */
  deleteSession: (sessionId: string) => Promise<void>;
}

const AgentSessionListActionsContext = createContext<
  AgentSessionListActionsContextValue | undefined
>(undefined);

interface AgentSessionListProviderProps {
  children: React.ReactNode;
}

/**
 * AgentSessionListProvider
 *
 * Manages the global list of agent sessions.
 */
export function AgentSessionListProvider({
  children,
}: AgentSessionListProviderProps) {
  const { modelId, provider } = useModelOptions();
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [isSessionsListLoading, setIsSessionsListLoading] = useState(false);

  /**
   * Load all agent sessions
   */
  const loadSessions = useCallback(async () => {
    logger.info('Loading all agent sessions');
    setIsSessionsListLoading(true);

    try {
      // Call Rust backend to get all sessions
      const response = await invoke<
        Array<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          agentConfig?: string;
          createdAt: number;
          updatedAt?: number;
        }>
      >('agent_get_all_sessions');

      const sessionList: AgentSession[] = response.map((s) => {
        let assistant: Assistant | undefined;
        if (s.agentConfig) {
          try {
            assistant = JSON.parse(s.agentConfig);
          } catch (e) {
            logger.error('Failed to parse agent config', e);
          }
        }

        return {
          id: s.id,
          name: s.name,
          status: s.status,
          assistant,
          createdAt: new Date(s.createdAt),
          updatedAt: s.updatedAt ? new Date(s.updatedAt) : undefined,
        };
      });

      // Sort by updated at desc (or created at desc)
      sessionList.sort((a, b) => {
        const timeA = a.updatedAt?.getTime() || a.createdAt.getTime();
        const timeB = b.updatedAt?.getTime() || b.createdAt.getTime();
        return timeB - timeA;
      });

      setSessions(sessionList);
      logger.info('Loaded sessions', { count: sessionList.length });
    } catch (err) {
      logger.error('Failed to load sessions', err);
      setSessions([]);
    } finally {
      setIsSessionsListLoading(false);
    }
  }, []);

  /**
   * Create a new agent session
   */
  const createSession = useCallback(
    async (params: CreateSessionParams): Promise<AgentSession> => {
      const { assistant, name } = params;

      logger.info('Creating new agent session', {
        assistantName: assistant.name,
        sessionName: name,
      });

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

        const session: AgentSession = {
          id: response.id,
          name: response.name,
          status: response.status || 'idle',
          createdAt: new Date(response.createdAt),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
        };

        // Add to list
        setSessions((prev) => [session, ...prev]);

        logger.info('Agent session created successfully', {
          sessionId: session.id,
        });

        return session;
      } catch (err) {
        logger.error('Failed to create agent session', err);
        throw err;
      }
    },
    [modelId, provider],
  );

  /**
   * Delete an agent session
   */
  const deleteSession = useCallback(async (sessionId: string) => {
    logger.info('Deleting agent session', { sessionId });

    try {
      await invoke('agent_delete_session', { sessionId });

      // Remove from sessions list
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));

      logger.info('Session deleted successfully', { sessionId });
    } catch (err) {
      logger.error('Failed to delete session', err);
      throw err;
    }
  }, []);

  // Initial load
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const stateValue = useMemo(
    () => ({
      sessions,
      isSessionsListLoading,
    }),
    [sessions, isSessionsListLoading],
  );

  const actionsValue = useMemo(
    () => ({
      createSession,
      loadSessions,
      deleteSession,
    }),
    [createSession, loadSessions, deleteSession],
  );

  return (
    <AgentSessionListStateContext.Provider value={stateValue}>
      <AgentSessionListActionsContext.Provider value={actionsValue}>
        {children}
      </AgentSessionListActionsContext.Provider>
    </AgentSessionListStateContext.Provider>
  );
}

export function useAgentSessionListState(): AgentSessionListStateContextValue {
  const context = useContext(AgentSessionListStateContext);
  if (!context) {
    throw new Error(
      'useAgentSessionListState must be used within AgentSessionListProvider',
    );
  }
  return context;
}

export function useAgentSessionListActions(): AgentSessionListActionsContextValue {
  const context = useContext(AgentSessionListActionsContext);
  if (!context) {
    throw new Error(
      'useAgentSessionListActions must be used within AgentSessionListProvider',
    );
  }
  return context;
}
