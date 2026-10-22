import { useCallback, useRef, useState } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { listAssistants } from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';
import { markStartupMilestone } from '@/lib/performance/startup-metrics';
import { sortSessionsByLatestActivity } from '@/lib/session-metadata';
import type {
  AgentSessionListCursor,
  AgentSessionListResponse,
  AgentSessionMetadata,
} from '@/models/agent-ipc';
import { dedupeSessionsById, pruneNotificationSessions } from './collections';
import {
  mapSessionMetadataList,
  normalizeSessionListResponse,
} from './mappings';
import {
  type InitialSessionListData,
  loadInitialSessionList,
  SESSION_LIST_PAGE_SIZE,
} from './startup-cache';
import type { SessionListStore } from './useSessionListStore';

const logger = getLogger('AgentSessionListContext');

type SessionListLoadersStore = Pick<
  SessionListStore,
  | 'setSessions'
  | 'setNotificationSessions'
  | 'sessionsRef'
  | 'notificationSessionsRef'
  | 'sessionListMutationVersionRef'
>;

/**
 * Loads the first page, further pages, and lazily expanded child sessions,
 * and tracks the loading flags for each.
 */
export function useSessionListLoaders({
  setSessions,
  setNotificationSessions,
  sessionsRef,
  notificationSessionsRef,
  sessionListMutationVersionRef,
}: SessionListLoadersStore) {
  const [isSessionsListLoading, setIsSessionsListLoading] = useState(false);
  const [isLoadingMoreSessions, setIsLoadingMoreSessions] = useState(false);
  const [hasMoreSessions, setHasMoreSessions] = useState(false);
  const [loadingChildrenParentIds, setLoadingChildrenParentIds] = useState<
    Set<string>
  >(() => new Set());
  const startupLoadRecordedRef = useRef(false);
  const latestSessionListPromiseRef =
    useRef<Promise<InitialSessionListData> | null>(null);
  const nextCursorRef = useRef<AgentSessionListCursor | undefined>(undefined);
  const childrenLoadInflightRef = useRef(new Map<string, Promise<void>>());
  const childrenLoadCompletedRef = useRef(new Set<string>());

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
      const isCurrentLoad = () =>
        mutationVersion === sessionListMutationVersionRef.current &&
        promise === latestSessionListPromiseRef.current;

      try {
        const [initialData, assistants] = await Promise.all([
          promise,
          listAssistants(),
        ]);

        if (!isCurrentLoad()) {
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

        const assistantsById = new Map(
          assistants.map((assistant) => [assistant.id, assistant]),
        );

        const recentSessions = mapSessionMetadataList(
          initialData.page.items,
          pendingApprovalCounts,
          assistantsById,
        );
        const unreadAttentionSessions = pruneNotificationSessions(
          mapSessionMetadataList(
            initialData.attentionSessions,
            pendingApprovalCounts,
            assistantsById,
          ),
        );

        sessionsRef.current = recentSessions;
        notificationSessionsRef.current = unreadAttentionSessions;
        setSessions(recentSessions);
        setNotificationSessions(unreadAttentionSessions);
        nextCursorRef.current = initialData.page.nextCursor;
        setHasMoreSessions(Boolean(initialData.page.nextCursor));
        childrenLoadCompletedRef.current.clear();
        childrenLoadInflightRef.current.clear();
        setLoadingChildrenParentIds(new Set());

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
        if (isCurrentLoad()) {
          sessionsRef.current = [];
          notificationSessionsRef.current = [];
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
    [
      notificationSessionsRef,
      sessionListMutationVersionRef,
      sessionsRef,
      setNotificationSessions,
      setSessions,
    ],
  );

  const loadMoreSessions = useCallback(async () => {
    const cursor = nextCursorRef.current;
    if (!cursor || isLoadingMoreSessions) {
      return;
    }

    setIsLoadingMoreSessions(true);
    try {
      const [response, assistants] = await Promise.all([
        safeInvoke<AgentSessionListResponse>('agent_list_sessions', {
          request: {
            cursor,
            limit: SESSION_LIST_PAGE_SIZE,
          },
        }),
        listAssistants(),
      ]);

      const normalizedResponse = normalizeSessionListResponse(response);

      const assistantsById = new Map(
        assistants.map((assistant) => [assistant.id, assistant]),
      );

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
          assistantsById,
        );

        const nextSessions = sortSessionsByLatestActivity(
          dedupeSessionsById([...previousSessions, ...incomingSessions]),
        );
        sessionsRef.current = nextSessions;
        return nextSessions;
      });
      nextCursorRef.current = normalizedResponse.nextCursor;
      setHasMoreSessions(Boolean(normalizedResponse.nextCursor));
    } catch (err) {
      logger.error('Failed to load more sessions', err);
      throw err;
    } finally {
      setIsLoadingMoreSessions(false);
    }
  }, [isLoadingMoreSessions, sessionsRef, setSessions]);

  const ensureChildrenLoaded = useCallback(
    async (parentSessionId: string) => {
      if (childrenLoadCompletedRef.current.has(parentSessionId)) {
        return;
      }

      const existing = childrenLoadInflightRef.current.get(parentSessionId);
      if (existing) {
        return existing;
      }

      setLoadingChildrenParentIds((previous) => {
        const next = new Set(previous);
        next.add(parentSessionId);
        return next;
      });

      const loadPromise = (async () => {
        try {
          const childMetadata = await safeInvoke<AgentSessionMetadata[]>(
            'agent_get_child_sessions',
            { sessionId: parentSessionId },
          );

          if (Array.isArray(childMetadata) && childMetadata.length > 0) {
            const assistantsById = new Map(
              (await listAssistants()).map((assistant) => [
                assistant.id,
                assistant,
              ]),
            );

            setSessions((previousSessions) => {
              const pendingApprovalCounts = new Map(
                previousSessions.map((session) => [
                  session.id,
                  session.pendingApprovalCount ?? 0,
                ]),
              );
              const incomingSessions = mapSessionMetadataList(
                childMetadata,
                pendingApprovalCounts,
                assistantsById,
              );
              const nextSessions = sortSessionsByLatestActivity(
                dedupeSessionsById([...previousSessions, ...incomingSessions]),
              );
              sessionsRef.current = nextSessions;
              return nextSessions;
            });
          }

          childrenLoadCompletedRef.current.add(parentSessionId);
        } catch (err) {
          logger.error('Failed to load child sessions', {
            parentSessionId,
            err,
          });
          throw err;
        } finally {
          childrenLoadInflightRef.current.delete(parentSessionId);
          setLoadingChildrenParentIds((previous) => {
            if (!previous.has(parentSessionId)) {
              return previous;
            }
            const next = new Set(previous);
            next.delete(parentSessionId);
            return next;
          });
        }
      })();

      childrenLoadInflightRef.current.set(parentSessionId, loadPromise);
      return loadPromise;
    },
    [sessionsRef, setSessions],
  );

  return {
    isSessionsListLoading,
    isLoadingMoreSessions,
    hasMoreSessions,
    loadingChildrenParentIds,
    loadSessions,
    loadMoreSessions,
    ensureChildrenLoaded,
  };
}
