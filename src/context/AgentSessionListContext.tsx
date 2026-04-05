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

import { listen } from '@tauri-apps/api/event';
import { matchPath, useLocation } from 'react-router-dom';
import { getLogger } from '../lib/logger';
import { useModelOptions } from './ModelProvider';
import { useBackendResource } from './GlobalEventContext';
import { AgentSession, CreateSessionParams } from '@/models/agent';
import { getAssistant } from '@/lib/backend/assistants';
import { Assistant } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useSettings } from '@/context/SettingsContext';
import { enforceRuntimeBuiltinAliases } from '@/lib/assistant/runtime-builtins';
import { useLLMService } from '@/context/LLMServiceContext';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type {
  AgentSessionMetadata,
  CreateAgentSessionRequest,
  AgentResponse,
  AgentConfig,
  WorkflowCompletionReason,
} from '@/models/agent-ipc';

const logger = getLogger('AgentSessionListContext');
const RESERVED_AGENT_SUBROUTES = new Set(['draft']);

// --- STATE CONTEXT ---
interface AgentSessionListStateContextValue {
  sessions: AgentSession[];
  notificationSessions: AgentSession[];
  unreadNotificationCount: number;
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

  /**
   * Delete only this session, orphaning its direct children as top-level sessions
   */
  deleteSessionOnly: (sessionId: string) => Promise<void>;

  /**
   * Toggle the bookmark flag on a session
   */
  toggleBookmark: (sessionId: string) => Promise<void>;

  /**
   * Persist that the user viewed a session and clear unread state locally.
   */
  markSessionViewed: (sessionId: string, viewedAt?: Date) => Promise<void>;

  /**
   * Remove one pending approval from the session after a response.
   */
  clearPendingApproval: (sessionId: string, toolCallId: string) => void;
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
  const location = useLocation();
  const { modelId, provider } = useModelOptions();
  const {
    value: { advanced },
  } = useSettings();
  const { clearSessionState } = useLLMService();
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [isSessionsListLoading, setIsSessionsListLoading] = useState(false);
  const pendingApprovalKeysRef = useRef(new Set<string>());
  const activeSessionId = useMemo(() => {
    const sessionId = matchPath('/agent/:sessionId', location.pathname)?.params
      .sessionId;
    if (!sessionId || RESERVED_AGENT_SUBROUTES.has(sessionId)) {
      return undefined;
    }
    return sessionId;
  }, [location.pathname]);

  const hasUnreadAttention = useCallback((session: AgentSession): boolean => {
    if (!session.lastAttentionAt || !session.lastAttentionReason) {
      return false;
    }

    if (!session.lastViewedAt) {
      return true;
    }

    return session.lastAttentionAt.getTime() > session.lastViewedAt.getTime();
  }, []);

  /**
   * Load all agent sessions
   */
  const loadSessions = useCallback(async () => {
    logger.info('Loading all agent sessions');
    setIsSessionsListLoading(true);

    try {
      // Call Rust backend to get all sessions
      const response = await safeInvoke<AgentSessionMetadata[]>(
        'agent_get_all_sessions',
      );

      const sessionList: AgentSession[] = response.map((s) => {
        let assistant: Assistant | undefined;
        let parentSessionId: string | undefined = s.parentSessionId;
        let lineageId: string | undefined = s.lineageId;
        let depth: number | undefined = s.depth;
        let orgId: string | undefined = s.orgId;
        let orgName: string | undefined = s.orgName;
        let orgRootSessionId: string | undefined = s.orgRootSessionId;

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
              orgId = orgId || readStringField(record, 'orgId', 'org_id');
              orgName =
                orgName || readStringField(record, 'orgName', 'org_name');
              orgRootSessionId =
                orgRootSessionId ||
                readStringField(
                  record,
                  'orgRootSessionId',
                  'org_root_session_id',
                );
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
          orgId,
          orgName,
          orgRootSessionId,
          createdAt: new Date(s.createdAt),
          updatedAt: s.updatedAt ? new Date(s.updatedAt) : undefined,
          lastViewedAt: s.lastViewedAt ? new Date(s.lastViewedAt) : undefined,
          lastMessageAt: s.lastMessageAt
            ? new Date(s.lastMessageAt)
            : undefined,
          lastAttentionAt: s.lastAttentionAt
            ? new Date(s.lastAttentionAt)
            : undefined,
          lastAttentionReason: s.lastAttentionReason,
          isBookmarked: s.isBookmarked ?? false,
          yoloMode: s.yoloMode ?? false,
          pendingApprovalCount: 0,
        };
      });
      // Sort by updated at desc (or created at desc)
      sessionList.sort((a, b) => {
        const timeA = a.updatedAt?.getTime() || a.createdAt.getTime();
        const timeB = b.updatedAt?.getTime() || b.createdAt.getTime();
        return timeB - timeA;
      });

      setSessions((prev) => {
        const pendingApprovalCounts = new Map(
          prev.map((session) => [
            session.id,
            session.pendingApprovalCount ?? 0,
          ]),
        );

        return sessionList.map((session) => ({
          ...session,
          pendingApprovalCount: pendingApprovalCounts.get(session.id) ?? 0,
        }));
      });
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
        // Explicitly casting to AgentConfig (IPC) to ensure compatibility
        const agentConfig: AgentConfig = {
          // Map Assistant fields to AgentConfig fields
          id: freshAssistant.id,
          name: freshAssistant.name,
          description: freshAssistant.description,
          systemPrompt: freshAssistant.systemPrompt,
          mcpServerIds: freshAssistant.mcpServerIds || [],
          localServices: freshAssistant.localServices || [],
          allowedBuiltInServiceAliases: enforceRuntimeBuiltinAliases(
            freshAssistant.allowedBuiltInServiceAliases,
          ),
          temperature: 1.0, // Default, not in Assistant model yet
          ...(advanced.defaultSessionMaxDepth > 0
            ? { maxDepth: advanced.defaultSessionMaxDepth }
            : {}),
          ...(advanced.defaultSessionMaxFanout > 0
            ? { maxFanout: advanced.defaultSessionMaxFanout }
            : {}),
        };

        // Generate session ID
        const sessionId = createId();

        const request: CreateAgentSessionRequest = {
          sessionId,
          name: name || `Conversation with ${assistant.name}`,
          model: modelId,
          provider: provider,
          agentConfig,
        };

        // Call Rust backend to create session
        const response = await safeInvoke<AgentSessionMetadata>(
          'agent_create_session',
          { request },
        );

        // Map back to internal AgentSession model (which uses Assistant type for config)
        // We use the sent agentConfig as the assistant base since we just built it
        const sessionAssistant: Assistant = {
          ...freshAssistant,
          // Re-apply runtime overrides if needed
        };

        const session: AgentSession = {
          id: response.id,
          name: response.name,
          status: response.status,
          model: response.model,
          provider: response.provider,
          assistant: sessionAssistant,
          parentSessionId: response.parentSessionId,
          lineageId:
            response.lineageId || response.parentSessionId || response.id,
          depth: response.depth ?? (response.parentSessionId ? 1 : 0),
          orgId: response.orgId,
          orgName: response.orgName,
          orgRootSessionId: response.orgRootSessionId,
          createdAt: new Date(response.createdAt),
          updatedAt: response.updatedAt
            ? new Date(response.updatedAt)
            : undefined,
          lastViewedAt: response.lastViewedAt
            ? new Date(response.lastViewedAt)
            : undefined,
          lastMessageAt: response.lastMessageAt
            ? new Date(response.lastMessageAt)
            : undefined,
          lastAttentionAt: response.lastAttentionAt
            ? new Date(response.lastAttentionAt)
            : undefined,
          lastAttentionReason: response.lastAttentionReason,
          yoloMode: response.yoloMode ?? false,
          pendingApprovalCount: 0,
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
   * Delete an agent session (cascade: removes all descendants too)
   */
  const deleteSession = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting agent session', { sessionId });

      try {
        const response = await safeInvoke<AgentResponse<string[]>>(
          'agent_delete_session',
          { sessionId },
        );
        const data = response?.data;
        let deletedIds: string[];

        if (
          Array.isArray(data) &&
          data.every((x): x is string => typeof x === 'string')
        ) {
          deletedIds = data;
        } else {
          logger.warn(
            'agent_delete_session returned unexpected data; falling back to single id',
            {
              sessionId,
              data,
            },
          );
          deletedIds = [sessionId];
        }

        const idsToRemove = new Set(deletedIds);

        // Remove the session and ALL its descendants from the UI using the authoritative list from Rust
        setSessions((prev) => prev.filter((s) => !idsToRemove.has(s.id)));

        // Clean up LLM state outside the updater to avoid double-invocation in StrictMode
        idsToRemove.forEach((id) => clearSessionState(id));

        logger.info('Session deleted successfully', { sessionId, deletedIds });
      } catch (err) {
        logger.error('Failed to delete session', err);
        throw err;
      }
    },
    [clearSessionState],
  );

  /**
   * Delete only this session, orphaning direct children as top-level sessions
   */
  const deleteSessionOnly = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting session only (orphaning children)', { sessionId });

      try {
        await safeInvoke<AgentResponse>('agent_delete_session_only', {
          sessionId,
        });

        clearSessionState(sessionId);

        // Remove the session; update direct children to have no parent
        setSessions((prev) =>
          prev
            .filter((s) => s.id !== sessionId)
            .map((s) =>
              s.parentSessionId === sessionId
                ? { ...s, parentSessionId: undefined }
                : s,
            ),
        );

        logger.info('Session deleted (children orphaned)', { sessionId });
      } catch (err) {
        logger.error('Failed to delete session only', err);
        throw err;
      }
    },
    [clearSessionState],
  );

  /**
   * Toggle the bookmark flag on a session (optimistic update)
   */
  const toggleBookmark = useCallback(
    async (sessionId: string) => {
      // Compute new value from current (non-optimistic) state before the update
      const session = sessions.find((s) => s.id === sessionId);
      const newValue = !(session?.isBookmarked ?? false);

      // Optimistic: set locally with the known new value
      setSessions((prev) =>
        prev.map((s) =>
          s.id === sessionId ? { ...s, isBookmarked: newValue } : s,
        ),
      );

      try {
        await safeInvoke<void>('agent_toggle_session_bookmark', {
          sessionId,
          bookmarked: newValue,
        });
      } catch (err) {
        logger.error('Failed to toggle bookmark', err);
        // Revert optimistic update on failure using the inverse of what we set
        setSessions((prev) =>
          prev.map((s) =>
            s.id === sessionId ? { ...s, isBookmarked: !newValue } : s,
          ),
        );
        throw err;
      }
    },
    [sessions],
  );

  const markSessionViewed = useCallback(
    async (sessionId: string, viewedAt = new Date()) => {
      await safeInvoke<void>('agent_mark_session_viewed', {
        sessionId,
        viewedAt: viewedAt.getTime(),
      });
      setSessions((prev) =>
        prev.map((session) =>
          session.id === sessionId
            ? applyViewedAtToSession(session, viewedAt)
            : session,
        ),
      );
    },
    [],
  );

  const clearPendingApproval = useCallback(
    (sessionId: string, toolCallId: string) => {
      pendingApprovalKeysRef.current.delete(`${sessionId}:${toolCallId}`);
      setSessions((prev) =>
        prev.map((session) =>
          session.id === sessionId
            ? {
                ...session,
                pendingApprovalCount: Math.max(
                  0,
                  (session.pendingApprovalCount ?? 0) - 1,
                ),
              }
            : session,
        ),
      );
    },
    [],
  );

  // Initial load
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Subscribe to agent:event for session resource updates via centralized hook
  useBackendResource('session', () => {
    logger.debug('Agent updated session resource, refreshing session list...');
    loadSessions();
  });

  useEffect(() => {
    if (!activeSessionId) {
      return;
    }

    const viewedAt = new Date();
    void markSessionViewed(activeSessionId, viewedAt).catch((err) => {
      logger.error('Failed to persist viewed state for active session', err);
    });
  }, [activeSessionId, markSessionViewed]);

  // Subscribe to lightweight agent events to keep session metadata fresh in place.
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<{
        type: string;
        sessionId?: string;
        status?: 'idle' | 'busy' | 'paused' | 'error';
        message?: {
          role: 'user' | 'assistant' | 'system' | 'tool';
          createdAt: number;
        };
        toolCallId?: string;
        approved?: boolean;
        reason?: WorkflowCompletionReason;
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
          return;
        }

        if (
          payload.type === 'messageAdded' &&
          payload.sessionId &&
          payload.message
        ) {
          const messageAt = new Date(payload.message.createdAt);
          const shouldMarkViewed = payload.sessionId === activeSessionId;

          setSessions((prev) =>
            prev.map((session) =>
              session.id === payload.sessionId
                ? {
                    ...session,
                    lastMessageAt: messageAt,
                    lastViewedAt: shouldMarkViewed
                      ? messageAt
                      : session.lastViewedAt,
                  }
                : session,
            ),
          );
          return;
        }

        if (
          payload.type === 'workflowCompleted' &&
          payload.sessionId &&
          payload.reason === 'recurringStop'
        ) {
          const attentionAt = new Date();
          const shouldMarkViewed = payload.sessionId === activeSessionId;

          setSessions((prev) =>
            prev.map((session) =>
              session.id === payload.sessionId
                ? shouldMarkViewed
                  ? applyViewedAtToSession(
                      {
                        ...session,
                        lastAttentionAt: attentionAt,
                        lastAttentionReason: 'recurringStop',
                      },
                      attentionAt,
                    )
                  : {
                      ...session,
                      lastAttentionAt: attentionAt,
                      lastAttentionReason: 'recurringStop',
                    }
                : session,
            ),
          );

          if (shouldMarkViewed) {
            void markSessionViewed(payload.sessionId, attentionAt).catch(
              (err) => {
                logger.error(
                  'Failed to mark active session viewed after recurring stop',
                  err,
                );
              },
            );
          }
          return;
        }

        if (
          payload.type === 'toolExecutionRequiresApproval' &&
          payload.sessionId &&
          payload.toolCallId
        ) {
          const pendingApprovalKey = `${payload.sessionId}:${payload.toolCallId}`;
          if (pendingApprovalKeysRef.current.has(pendingApprovalKey)) {
            return;
          }

          const attentionAt = new Date();
          const shouldMarkViewed = payload.sessionId === activeSessionId;
          pendingApprovalKeysRef.current.add(pendingApprovalKey);
          setSessions((prev) =>
            prev.map((session) =>
              session.id === payload.sessionId
                ? shouldMarkViewed
                  ? applyViewedAtToSession(
                      {
                        ...session,
                        lastAttentionAt: attentionAt,
                        lastAttentionReason: 'pendingApproval',
                        pendingApprovalCount:
                          (session.pendingApprovalCount ?? 0) + 1,
                      },
                      attentionAt,
                    )
                  : {
                      ...session,
                      lastAttentionAt: attentionAt,
                      lastAttentionReason: 'pendingApproval',
                      pendingApprovalCount:
                        (session.pendingApprovalCount ?? 0) + 1,
                    }
                : session,
            ),
          );

          if (shouldMarkViewed) {
            void markSessionViewed(payload.sessionId, attentionAt).catch(
              (err) => {
                logger.error(
                  'Failed to mark active session viewed after approval request',
                  err,
                );
              },
            );
          }
          return;
        }

        if (
          payload.type === 'toolExecutionApprovalResolved' &&
          payload.sessionId &&
          payload.toolCallId
        ) {
          clearPendingApproval(payload.sessionId, payload.toolCallId);
        }
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [activeSessionId, markSessionViewed]);

  const notificationSessions = useMemo(
    () =>
      sessions
        .filter((session) => hasUnreadAttention(session))
        .slice()
        .sort((left, right) => {
          const leftPending = left.pendingApprovalCount ?? 0;
          const rightPending = right.pendingApprovalCount ?? 0;
          if (leftPending !== rightPending) {
            return rightPending - leftPending;
          }

          const leftTime =
            left.lastAttentionAt?.getTime() ??
            left.lastMessageAt?.getTime() ??
            left.updatedAt?.getTime() ??
            left.createdAt.getTime();
          const rightTime =
            right.lastAttentionAt?.getTime() ??
            right.lastMessageAt?.getTime() ??
            right.updatedAt?.getTime() ??
            right.createdAt.getTime();

          return rightTime - leftTime;
        }),
    [hasUnreadAttention, sessions],
  );

  const stateValue = useMemo(
    () => ({
      sessions,
      notificationSessions,
      unreadNotificationCount: notificationSessions.length,
      isSessionsListLoading,
    }),
    [sessions, notificationSessions, isSessionsListLoading],
  );

  const actionsValue = useMemo(
    () => ({
      createSession,
      loadSessions,
      deleteSession,
      deleteSessionOnly,
      toggleBookmark,
      markSessionViewed,
      clearPendingApproval,
    }),
    [
      createSession,
      loadSessions,
      deleteSession,
      deleteSessionOnly,
      toggleBookmark,
      markSessionViewed,
      clearPendingApproval,
    ],
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
