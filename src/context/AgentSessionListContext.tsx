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
import { useBackendResource } from './GlobalEventContext';
import { AgentSession, CreateSessionParams } from '@/models/agent';
import { getAssistant } from '@/lib/backend/assistants';
import { Assistant } from '@/models/chat';

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
          model: string;
          provider: string;
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
          model: s.model,
          provider: s.provider,
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
        // ✅ CRITICAL FIX: Reload assistant from DB to get latest configuration
        // This ensures that any recent changes (e.g., built-in tool updates)
        // are included in the session config
        if (!assistant.id) {
          throw new Error('Assistant ID is required to create session');
        }

        const freshAssistant = await getAssistant(assistant.id);

        if (!freshAssistant) {
          throw new Error(`Assistant ${assistant.id} not found in database`);
        }

        logger.debug('Reloaded assistant from DB', {
          assistantId: freshAssistant.id,
          allowedBuiltInServiceAliases:
            freshAssistant.allowedBuiltInServiceAliases,
        });

        // Build agent config from fresh assistant data
        const agentConfig: Assistant = {
          ...freshAssistant,
        };

        // Generate session ID
        const { createId } = await import('@paralleldrive/cuid2');
        const sessionId = createId();

        // Call Rust backend to create session
        const response = await invoke<{
          id: string;
          name?: string;
          status: 'idle' | 'busy' | 'paused' | 'error';
          model: string;
          provider: string;
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
          status: response.status,
          model: response.model,
          provider: response.provider,
          assistant: agentConfig,
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

  // Subscribe to agent:event for session resource updates via centralized hook
  useBackendResource('session', () => {
    logger.debug('Agent updated session resource, refreshing session list...');
    loadSessions();
  });

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
