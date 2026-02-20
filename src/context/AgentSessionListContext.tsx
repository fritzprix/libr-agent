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
import { useModelOptions } from './ModelProvider';
import { useBackendResource } from './GlobalEventContext';
import { AgentSession, CreateSessionParams } from '@/models/agent';
import { getAssistant } from '@/lib/backend/assistants';
import { Assistant } from '@/models/chat';
import { useSettings } from '@/context/SettingsContext';

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
  const {
    value: { advanced },
  } = useSettings();
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
          parentSessionId?: string;
          lineageId?: string;
          depth?: number;
          createdAt: number;
          updatedAt?: number;
        }>
      >('agent_get_all_sessions');

      const sessionList: AgentSession[] = response.map((s) => {
        let assistant: Assistant | undefined;
        let parentSessionId: string | undefined = s.parentSessionId;
        let lineageId: string | undefined = s.lineageId;
        let depth: number | undefined = s.depth;

        const readStringField = (
          record: Record<string, unknown>,
          ...keys: string[]
        ): string | undefined => {
          for (const key of keys) {
            const value = record[key];
            if (typeof value === 'string' && value.length > 0) {
              return value;
            }
          }
          return undefined;
        };

        const readNumberField = (
          record: Record<string, unknown>,
          ...keys: string[]
        ): number | undefined => {
          for (const key of keys) {
            const value = record[key];
            if (typeof value === 'number' && Number.isFinite(value)) {
              return value;
            }
          }
          return undefined;
        };

        if (s.agentConfig) {
          try {
            const parsed = JSON.parse(s.agentConfig) as unknown;
            if (typeof parsed === 'object' && parsed !== null) {
              const record = parsed as Record<string, unknown>;
              parentSessionId =
                parentSessionId ||
                readStringField(record, 'parentSessionId', 'parent_session_id');
              lineageId =
                lineageId || readStringField(record, 'lineageId', 'lineage_id');
              depth = depth ?? readNumberField(record, 'depth');
              assistant = parsed as Assistant;
            }
          } catch (e) {
            logger.error('Failed to parse agent config', e);
          }
        }

        if (!lineageId) {
          lineageId = parentSessionId || s.id;
        }

        if (depth === undefined) {
          depth = parentSessionId ? 1 : 0;
        }

        return {
          id: s.id,
          name: s.name,
          status: s.status,
          model: s.model,
          provider: s.provider,
          assistant,
          parentSessionId,
          lineageId,
          depth,
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
        const agentConfig: Assistant & {
          maxDepth?: number;
          maxFanout?: number;
        } = {
          ...freshAssistant,
          ...(advanced.defaultSessionMaxDepth > 0
            ? { maxDepth: advanced.defaultSessionMaxDepth }
            : {}),
          ...(advanced.defaultSessionMaxFanout > 0
            ? { maxFanout: advanced.defaultSessionMaxFanout }
            : {}),
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
          parentSessionId?: string;
          lineageId?: string;
          depth?: number;
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
          parentSessionId: response.parentSessionId,
          lineageId:
            response.lineageId || response.parentSessionId || response.id,
          depth: response.depth ?? (response.parentSessionId ? 1 : 0),
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
    [
      advanced.defaultSessionMaxDepth,
      advanced.defaultSessionMaxFanout,
      modelId,
      provider,
    ],
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

  // Subscribe to statusChanged events to update session status in-place
  // (avoids full reload on every status transition during normal workflow)
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<{
        type: string;
        sessionId?: string;
        status?: 'idle' | 'busy' | 'paused' | 'error';
      }>('agent:event', (event) => {
        const payload = event.payload;
        if (
          payload.type === 'statusChanged' &&
          payload.sessionId &&
          payload.status
        ) {
          setSessions((prev) =>
            prev.map((s) =>
              s.id === payload.sessionId
                ? { ...s, status: payload.status as AgentSession['status'] }
                : s,
            ),
          );
        }
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

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
