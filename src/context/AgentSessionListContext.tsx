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
import type { AgentSession, CreateSessionParams } from '@/models/agent';
import { getAssistant } from '@/lib/backend/assistants';
import type { Assistant } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import { useSettings } from '@/context/SettingsContext';
import { enforceRuntimeBuiltinAliases } from '@/lib/assistant/runtime-builtins';
import { useLLMService } from '@/context/LLMServiceContext';
import { markStartupMilestone } from '@/lib/performance/startup-metrics';
import {
  mapSessionMetadataToAgentSession,
  sortSessionsByLatestActivity,
} from '@/lib/session-metadata';
import { applyViewedAtToSession } from '@/lib/session-utils';
import type {
  AgentSessionMetadata,
  AgentSessionListCursor,
  AgentSessionListResponse,
  CreateAgentSessionRequest,
  AgentResponse,
  AgentConfig,
  WorkflowCompletionReason,
} from '@/models/agent-ipc';

const logger = getLogger('AgentSessionListContext');
const RESERVED_AGENT_SUBROUTES = new Set(['draft']);
const SESSION_LIST_PAGE_SIZE = 100;

interface InitialSessionListData {
  page: AgentSessionListResponse;
  attentionSessions: AgentSessionMetadata[];
}

let cachedInitialSessionListData: InitialSessionListData | null = null;
let cachedInitialSessionListPromise: Promise<InitialSessionListData> | null =
  null;
let sessionListCacheGeneration = 0;

type SessionMetadataLoadSource = 'cache' | 'inflight' | 'network';

interface SessionMetadataLoadHandle {
  source: SessionMetadataLoadSource;
  promise: Promise<InitialSessionListData>;
}

function invalidateSessionListStartupCache() {
  sessionListCacheGeneration += 1;
  cachedInitialSessionListData = null;
  cachedInitialSessionListPromise = null;
}

function loadInitialSessionList(
  forceRefresh = false,
): SessionMetadataLoadHandle {
  if (forceRefresh) {
    invalidateSessionListStartupCache();
  }

  if (!forceRefresh && cachedInitialSessionListData !== null) {
    return {
      source: 'cache',
      promise: Promise.resolve(cachedInitialSessionListData),
    };
  }

  if (!forceRefresh && cachedInitialSessionListPromise) {
    return {
      source: 'inflight',
      promise: cachedInitialSessionListPromise,
    };
  }

  const requestGeneration = sessionListCacheGeneration;
  const request = Promise.all([
    safeInvoke<AgentSessionListResponse>('agent_list_sessions', {
      request: { limit: SESSION_LIST_PAGE_SIZE },
    }),
    safeInvoke<AgentSessionMetadata[]>('agent_list_attention_sessions'),
  ])
    .then(([page, attentionSessions]) => {
      const normalizedPage = Array.isArray(page)
        ? {
            items: page,
            nextCursor: undefined,
          }
        : page && Array.isArray(page.items)
          ? page
          : {
              items: [],
              nextCursor: undefined,
            };
      const initialData: InitialSessionListData = {
        page: normalizedPage,
        attentionSessions: Array.isArray(attentionSessions)
          ? attentionSessions
          : [],
      };
      if (
        requestGeneration === sessionListCacheGeneration &&
        cachedInitialSessionListPromise === request
      ) {
        cachedInitialSessionListData = initialData;
      }
      return initialData;
    })
    .finally(() => {
      if (cachedInitialSessionListPromise === request) {
        cachedInitialSessionListPromise = null;
      }
    });

  cachedInitialSessionListPromise = request;
  return {
    source: 'network',
    promise: request,
  };
}

export function __resetAgentSessionListStartupCacheForTests() {
  invalidateSessionListStartupCache();
}

function dedupeSessionsById(sessions: AgentSession[]): AgentSession[] {
  const sessionById = new Map<string, AgentSession>();
  sessions.forEach((session) => {
    sessionById.set(session.id, session);
  });
  return Array.from(sessionById.values());
}

// --- STATE CONTEXT ---
interface AgentSessionListStateContextValue {
  sessions: AgentSession[];
  notificationSessions: AgentSession[];
  unreadNotificationCount: number;
  isSessionsListLoading: boolean;
  hasMoreSessions: boolean;
  isLoadingMoreSessions: boolean;
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
   * Load the first page of recent agent sessions.
   */
  loadSessions: (forceRefresh?: boolean) => Promise<void>;

  /**
   * Load the next page of session history.
   */
  loadMoreSessions: () => Promise<void>;

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
  const [notificationSessions, setNotificationSessions] = useState<
    AgentSession[]
  >([]);
  const [isSessionsListLoading, setIsSessionsListLoading] = useState(false);
  const [isLoadingMoreSessions, setIsLoadingMoreSessions] = useState(false);
  const [hasMoreSessions, setHasMoreSessions] = useState(false);
  const sessionsRef = useRef<AgentSession[]>([]);
  const notificationSessionsRef = useRef<AgentSession[]>([]);
  const pendingApprovalKeysRef = useRef(new Set<string>());
  const startupLoadRecordedRef = useRef(false);
  const sessionListMutationVersionRef = useRef(0);
  const latestSessionListPromiseRef =
    useRef<Promise<InitialSessionListData> | null>(null);
  const nextCursorRef = useRef<AgentSessionListCursor | undefined>(undefined);
  const activeSessionId = useMemo(() => {
    const sessionId = matchPath('/agent/:sessionId', location.pathname)?.params
      .sessionId;
    if (!sessionId || RESERVED_AGENT_SUBROUTES.has(sessionId)) {
      return undefined;
    }
    return sessionId;
  }, [location.pathname]);
  sessionsRef.current = sessions;
  notificationSessionsRef.current = notificationSessions;

  const hasUnreadAttention = useCallback((session: AgentSession): boolean => {
    if (!session.lastAttentionAt || !session.lastAttentionReason) {
      return false;
    }

    if (!session.lastViewedAt) {
      return true;
    }

    return session.lastAttentionAt.getTime() > session.lastViewedAt.getTime();
  }, []);

  const sortNotificationSessions = useCallback(
    (items: AgentSession[]) =>
      items.slice().sort((left, right) => {
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
    [],
  );

  const pruneNotificationSessions = useCallback(
    (items: AgentSession[]) =>
      sortNotificationSessions(
        dedupeSessionsById(items).filter((session) =>
          hasUnreadAttention(session),
        ),
      ),
    [hasUnreadAttention, sortNotificationSessions],
  );

  const updateSessionInList = useCallback(
    (
      items: AgentSession[],
      sessionId: string,
      updater: (session: AgentSession) => AgentSession,
    ): AgentSession[] =>
      items.map((session) =>
        session.id === sessionId ? updater(session) : session,
      ),
    [],
  );

  const mutateSessions = useCallback(
    (
      updater: (previousSessions: AgentSession[]) => AgentSession[],
      options?: {
        notificationUpdater?: (
          previousNotifications: AgentSession[],
        ) => AgentSession[];
      },
    ) => {
      sessionListMutationVersionRef.current += 1;
      invalidateSessionListStartupCache();
      setSessions((previousSessions) => {
        const nextSessions = updater(previousSessions);
        sessionsRef.current = nextSessions;
        return nextSessions;
      });
      if (options?.notificationUpdater) {
        setNotificationSessions((previousNotifications) => {
          const nextNotifications = pruneNotificationSessions(
            options.notificationUpdater?.(previousNotifications) ??
              previousNotifications,
          );
          notificationSessionsRef.current = nextNotifications;
          return nextNotifications;
        });
      }
    },
    [pruneNotificationSessions],
  );

  const applySessionUpdate = useCallback(
    (sessionId: string, updater: (session: AgentSession) => AgentSession) => {
      let updatedSessionFromSessions: AgentSession | undefined;

      setSessions((previousSessions) => {
        const nextSessions = updateSessionInList(
          previousSessions,
          sessionId,
          (session) => {
            const nextSession = updater(session);
            updatedSessionFromSessions = nextSession;
            return nextSession;
          },
        );
        sessionsRef.current = nextSessions;
        return nextSessions;
      });
      setNotificationSessions((previousNotifications) => {
        const existingNotification = previousNotifications.find(
          (session) => session.id === sessionId,
        );
        const nextNotificationSession =
          updatedSessionFromSessions ??
          (existingNotification ? updater(existingNotification) : undefined);
        const nextNotifications = pruneNotificationSessions(
          nextNotificationSession
            ? existingNotification
              ? previousNotifications.map((session) =>
                  session.id === sessionId ? nextNotificationSession : session,
                )
              : hasUnreadAttention(nextNotificationSession)
                ? [...previousNotifications, nextNotificationSession]
                : previousNotifications
            : previousNotifications,
        );
        notificationSessionsRef.current = nextNotifications;
        return nextNotifications;
      });
    },
    [hasUnreadAttention, pruneNotificationSessions, updateSessionInList],
  );

  const mapSessionMetadataList = useCallback(
    (
      sessionMetadataList: AgentSessionMetadata[],
      pendingApprovalCounts: Map<string, number>,
    ) =>
      sortSessionsByLatestActivity(
        sessionMetadataList.map((sessionMetadata) =>
          mapSessionMetadataToAgentSession(
            sessionMetadata,
            pendingApprovalCounts.get(sessionMetadata.id) ?? 0,
          ),
        ),
      ),
    [],
  );

  /**
   * Load the first page of recent agent sessions plus notification sessions.
   */
  const loadSessions = useCallback(
    async (forceRefresh = false) => {
      const { source, promise } = loadInitialSessionList(forceRefresh);
      latestSessionListPromiseRef.current = promise;
      const shouldLogLoad = forceRefresh || source === 'network';

      if (shouldLogLoad) {
        logger.info('Loading recent agent sessions');
      }

      setIsSessionsListLoading(true);
      const mutationVersion = sessionListMutationVersionRef.current;

      try {
        const initialData = await promise;

        if (
          mutationVersion !== sessionListMutationVersionRef.current ||
          promise !== latestSessionListPromiseRef.current
        ) {
          return;
        }

        const pendingApprovalCounts = new Map<string, number>();
        [...sessionsRef.current, ...notificationSessionsRef.current].forEach(
          (session) => {
            pendingApprovalCounts.set(
              session.id,
              session.pendingApprovalCount ?? 0,
            );
          },
        );

        const recentSessions = mapSessionMetadataList(
          initialData.page.items,
          pendingApprovalCounts,
        );
        const unreadAttentionSessions = pruneNotificationSessions(
          mapSessionMetadataList(
            initialData.attentionSessions,
            pendingApprovalCounts,
          ),
        );

        setSessions(recentSessions);
        setNotificationSessions(unreadAttentionSessions);
        nextCursorRef.current = initialData.page.nextCursor;
        setHasMoreSessions(Boolean(initialData.page.nextCursor));

        if (shouldLogLoad) {
          logger.info('Loaded recent sessions', {
            count: recentSessions.length,
            notificationCount: unreadAttentionSessions.length,
            hasMore: Boolean(initialData.page.nextCursor),
          });
        }
      } catch (err) {
        if (shouldLogLoad) {
          logger.error('Failed to load recent sessions', err);
        }
        if (
          mutationVersion === sessionListMutationVersionRef.current &&
          promise === latestSessionListPromiseRef.current
        ) {
          setSessions([]);
          setNotificationSessions([]);
          nextCursorRef.current = undefined;
          setHasMoreSessions(false);
        }
      } finally {
        if (promise === latestSessionListPromiseRef.current) {
          setIsSessionsListLoading(false);
        }
        if (
          promise === latestSessionListPromiseRef.current &&
          !startupLoadRecordedRef.current
        ) {
          startupLoadRecordedRef.current = true;
          markStartupMilestone('session-list-settled');
        }
      }
    },
    [mapSessionMetadataList, pruneNotificationSessions],
  );

  const loadMoreSessions = useCallback(async () => {
    const cursor = nextCursorRef.current;
    if (!cursor || isLoadingMoreSessions) {
      return;
    }

    setIsLoadingMoreSessions(true);
    try {
      const response = await safeInvoke<AgentSessionListResponse>(
        'agent_list_sessions',
        {
          request: {
            cursor,
            limit: SESSION_LIST_PAGE_SIZE,
          },
        },
      );

      const normalizedResponse =
        response && Array.isArray(response.items)
          ? response
          : { items: [], nextCursor: undefined };

      setSessions((previousSessions) => {
        const pendingApprovalCounts = new Map(
          previousSessions.map((session) => [
            session.id,
            session.pendingApprovalCount ?? 0,
          ]),
        );
        const incomingSessions = mapSessionMetadataList(
          normalizedResponse.items,
          pendingApprovalCounts,
        );

        return sortSessionsByLatestActivity(
          dedupeSessionsById([...previousSessions, ...incomingSessions]),
        );
      });
      nextCursorRef.current = normalizedResponse.nextCursor;
      setHasMoreSessions(Boolean(normalizedResponse.nextCursor));
    } catch (err) {
      logger.error('Failed to load more sessions', err);
      throw err;
    } finally {
      setIsLoadingMoreSessions(false);
    }
  }, [isLoadingMoreSessions, mapSessionMetadataList]);

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
        mutateSessions((prev) => [session, ...prev]);

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
      mutateSessions,
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
        mutateSessions((prev) => prev.filter((s) => !idsToRemove.has(s.id)), {
          notificationUpdater: (prev) =>
            prev.filter((s) => !idsToRemove.has(s.id)),
        });

        // Clean up LLM state outside the updater to avoid double-invocation in StrictMode
        idsToRemove.forEach((id) => clearSessionState(id));

        logger.info('Session deleted successfully', { sessionId, deletedIds });
      } catch (err) {
        logger.error('Failed to delete session', err);
        throw err;
      }
    },
    [clearSessionState, mutateSessions],
  );

  /**
   * Delete only this session, orphaning direct children as top-level sessions
   */
  const deleteSessionOnly = useCallback(
    async (sessionId: string) => {
      logger.info('Deleting session only (orphaning children)', { sessionId });

      try {
        const response = await safeInvoke<
          AgentResponse<{ deletedId: string; orphanedIds: string[] }>
        >('agent_delete_session_only', {
          sessionId,
        });

        const actualDeletedId = response?.data?.deletedId || sessionId;
        clearSessionState(actualDeletedId);

        const orphanedIds = new Set(response?.data?.orphanedIds || []);

        // Remove the session; update explicitly orphaned children to have no parent
        mutateSessions(
          (prev) =>
            prev
              .filter((s) => s.id !== actualDeletedId)
              .map((s) =>
                orphanedIds.has(s.id)
                  ? { ...s, parentSessionId: undefined }
                  : s,
              ),
          {
            notificationUpdater: (prev) =>
              prev
                .filter((s) => s.id !== actualDeletedId)
                .map((s) =>
                  orphanedIds.has(s.id)
                    ? { ...s, parentSessionId: undefined }
                    : s,
                ),
          },
        );

        logger.info('Session deleted (children orphaned)', { sessionId });
      } catch (err) {
        logger.error('Failed to delete session only', err);
        throw err;
      }
    },
    [clearSessionState, mutateSessions],
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
      mutateSessions(
        (prev) =>
          prev.map((s) =>
            s.id === sessionId ? { ...s, isBookmarked: newValue } : s,
          ),
        {
          notificationUpdater: (prev) =>
            prev.map((s) =>
              s.id === sessionId ? { ...s, isBookmarked: newValue } : s,
            ),
        },
      );

      try {
        await safeInvoke<void>('agent_toggle_session_bookmark', {
          sessionId,
          bookmarked: newValue,
        });
      } catch (err) {
        logger.error('Failed to toggle bookmark', err);
        // Revert optimistic update on failure using the inverse of what we set
        applySessionUpdate(sessionId, (session) => ({
          ...session,
          isBookmarked: !newValue,
        }));
        throw err;
      }
    },
    [applySessionUpdate, mutateSessions, sessions],
  );

  const markSessionViewed = useCallback(
    async (sessionId: string, viewedAt = new Date()) => {
      await safeInvoke<void>('agent_mark_session_viewed', {
        sessionId,
        viewedAt: viewedAt.getTime(),
      });
      applySessionUpdate(sessionId, (session) =>
        applyViewedAtToSession(session, viewedAt),
      );
    },
    [applySessionUpdate],
  );

  const clearPendingApproval = useCallback(
    (sessionId: string, toolCallId: string) => {
      pendingApprovalKeysRef.current.delete(`${sessionId}:${toolCallId}`);
      applySessionUpdate(sessionId, (session) => ({
        ...session,
        pendingApprovalCount: Math.max(
          0,
          (session.pendingApprovalCount ?? 0) - 1,
        ),
      }));
    },
    [applySessionUpdate],
  );

  // Initial load
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Subscribe to agent:event for session resource updates via centralized hook
  useBackendResource('session', () => {
    logger.debug('Agent updated session resource, refreshing session list...');
    loadSessions(true);
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
          applySessionUpdate(payload.sessionId, (session) => ({
            ...session,
            status: payload.status as AgentSession['status'],
          }));
          return;
        }

        if (
          payload.type === 'messageAdded' &&
          payload.sessionId &&
          payload.message
        ) {
          const messageAt = new Date(payload.message.createdAt);
          const shouldMarkViewed = payload.sessionId === activeSessionId;

          applySessionUpdate(payload.sessionId, (session) => ({
            ...session,
            lastMessageAt: messageAt,
            lastViewedAt: shouldMarkViewed ? messageAt : session.lastViewedAt,
          }));
          return;
        }

        if (
          payload.type === 'workflowCompleted' &&
          payload.sessionId &&
          payload.reason === 'recurringStop'
        ) {
          const attentionAt = new Date();
          const shouldMarkViewed = payload.sessionId === activeSessionId;

          applySessionUpdate(payload.sessionId, (session) =>
            shouldMarkViewed
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
                },
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
          applySessionUpdate(payload.sessionId, (session) =>
            shouldMarkViewed
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
                  pendingApprovalCount: (session.pendingApprovalCount ?? 0) + 1,
                },
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
  }, [
    activeSessionId,
    applySessionUpdate,
    clearPendingApproval,
    markSessionViewed,
  ]);

  const stateValue = useMemo(
    () => ({
      sessions,
      notificationSessions,
      unreadNotificationCount: notificationSessions.length,
      isSessionsListLoading,
      hasMoreSessions,
      isLoadingMoreSessions,
    }),
    [
      hasMoreSessions,
      isLoadingMoreSessions,
      isSessionsListLoading,
      notificationSessions,
      sessions,
    ],
  );

  const actionsValue = useMemo(
    () => ({
      createSession,
      loadSessions,
      loadMoreSessions,
      deleteSession,
      deleteSessionOnly,
      toggleBookmark,
      markSessionViewed,
      clearPendingApproval,
    }),
    [
      createSession,
      loadSessions,
      loadMoreSessions,
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
