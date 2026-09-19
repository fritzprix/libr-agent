import { useCallback, useEffect, useRef, useState } from 'react';
import { listAssistants } from '@/lib/backend/assistants';
import { safeInvoke } from '@/lib/backend/core';
import { useDebouncedValue } from '@/features/knowledge/hooks/useDebouncedValue';
import { getLogger } from '@/lib/logger';
import type { AgentSession } from '@/models/agent';
import type { Assistant } from '@/models/chat';
import type {
  AgentSessionListCursor,
  AgentSessionListRequest,
  AgentSessionListResponse,
  AgentSessionMetadata,
} from '@/models/agent-ipc';
import {
  mapSessionMetadataList,
  normalizeSessionListResponse,
} from '@/context/agent-session-list/mappings';
import { dedupeSessionsById } from '@/context/agent-session-list/collections';
import { sortSessionsByLatestActivity } from '@/lib/session-metadata';
import { SESSION_LIST_PAGE_SIZE } from '@/context/agent-session-list/startup-cache';
import type { SessionStatus } from '@/lib/session-utils';

const logger = getLogger('useSessionHistorySearch');
const SEARCH_DEBOUNCE_MS = 300;

interface UseSessionHistorySearchArgs {
  searchQuery: string;
  bookmarkedOnly?: boolean;
  statusFilter?: 'all' | SessionStatus;
  browseSessions: AgentSession[];
  browseHasMore: boolean;
  browseLoading: boolean;
  browseLoadingMore: boolean;
  onBrowseLoadMore: () => void;
  /** Bump to re-run the active server search (e.g. refresh button). */
  refreshToken?: number;
}

interface UseSessionHistorySearchResult {
  displaySessions: AgentSession[];
  hasMoreSessions: boolean;
  isLoading: boolean;
  isLoadingMore: boolean;
  /** Empty when server already filtered; keeps client text filter from dropping matches. */
  clientSearchQuery: string;
  isServerSearchActive: boolean;
  loadMore: () => void | Promise<void>;
  /** Load direct children into the active search result set. */
  ensureSearchChildrenLoaded: (parentSessionId: string) => Promise<void>;
  /** Remove a session and any loaded descendants from search results. */
  removeSearchSessionTree: (sessionId: string) => void;
  /** Remove one session and clear parent links on loaded children. */
  removeSearchSessionOnly: (sessionId: string) => void;
  /** Optimistically update bookmark flag in search results. */
  setSearchSessionBookmarked: (sessionId: string, bookmarked: boolean) => void;
}

function collectSubtreeIds(
  sessions: AgentSession[],
  rootId: string,
): Set<string> {
  const idsToRemove = new Set<string>([rootId]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const session of sessions) {
      if (
        !idsToRemove.has(session.id) &&
        session.parentSessionId &&
        idsToRemove.has(session.parentSessionId)
      ) {
        idsToRemove.add(session.id);
        grew = true;
      }
    }
  }
  return idsToRemove;
}

function buildSearchRequest({
  search,
  cursor,
  bookmarkedOnly,
  statusFilter,
}: {
  search: string;
  cursor?: AgentSessionListCursor;
  bookmarkedOnly: boolean;
  statusFilter: 'all' | SessionStatus;
}): AgentSessionListRequest {
  const request: AgentSessionListRequest = {
    limit: SESSION_LIST_PAGE_SIZE,
    search,
  };
  if (cursor) {
    request.cursor = cursor;
  }
  if (bookmarkedOnly) {
    request.bookmarkedOnly = true;
  }
  if (statusFilter !== 'all') {
    request.status = statusFilter;
  }
  return request;
}

export function useSessionHistorySearch({
  searchQuery,
  bookmarkedOnly = false,
  statusFilter = 'all',
  browseSessions,
  browseHasMore,
  browseLoading,
  browseLoadingMore,
  onBrowseLoadMore,
  refreshToken = 0,
}: UseSessionHistorySearchArgs): UseSessionHistorySearchResult {
  const debouncedSearchQuery = useDebouncedValue(searchQuery, SEARCH_DEBOUNCE_MS);
  const trimmedInput = searchQuery.trim();
  const trimmedDebounced = debouncedSearchQuery.trim();
  const isDebouncing = trimmedInput !== trimmedDebounced;
  /** True once the user typed a query — even before debounce settles. */
  const wantsServerSearch = trimmedInput.length > 0;
  const isServerSearchActive = trimmedDebounced.length > 0;

  const [searchSessions, setSearchSessions] = useState<AgentSession[]>([]);
  const [searchCursor, setSearchCursor] = useState<
    AgentSessionListCursor | undefined
  >(undefined);
  const [searchHasMore, setSearchHasMore] = useState(false);
  const [isSearchLoading, setIsSearchLoading] = useState(false);
  const [isSearchLoadingMore, setIsSearchLoadingMore] = useState(false);
  const [activeSearchQuery, setActiveSearchQuery] = useState('');

  const requestGenerationRef = useRef(0);
  const searchCursorRef = useRef<AgentSessionListCursor | undefined>(undefined);
  const isSearchLoadingMoreRef = useRef(false);
  const assistantsByIdRef = useRef<Map<string, Assistant>>(new Map());
  const childrenLoadCompletedRef = useRef(new Set<string>());
  const childrenLoadInflightRef = useRef(new Map<string, Promise<void>>());

  useEffect(() => {
    searchCursorRef.current = searchCursor;
  }, [searchCursor]);

  const resetSearchLoadingMore = useCallback(() => {
    isSearchLoadingMoreRef.current = false;
    setIsSearchLoadingMore(false);
  }, []);

  useEffect(() => {
    if (!trimmedDebounced) {
      requestGenerationRef.current += 1;
      assistantsByIdRef.current = new Map();
      childrenLoadCompletedRef.current = new Set();
      childrenLoadInflightRef.current = new Map();
      setSearchSessions([]);
      setSearchCursor(undefined);
      setSearchHasMore(false);
      setIsSearchLoading(false);
      resetSearchLoadingMore();
      setActiveSearchQuery('');
      return;
    }

    const generation = ++requestGenerationRef.current;
    childrenLoadCompletedRef.current = new Set();
    childrenLoadInflightRef.current = new Map();
    setIsSearchLoading(true);
    resetSearchLoadingMore();
    setActiveSearchQuery(trimmedDebounced);

    const request = buildSearchRequest({
      search: trimmedDebounced,
      bookmarkedOnly,
      statusFilter,
    });

    void (async () => {
      try {
        const response = await safeInvoke<AgentSessionListResponse>(
          'agent_list_sessions',
          { request },
        );
        if (generation !== requestGenerationRef.current) {
          return;
        }

        const normalized = normalizeSessionListResponse(response);
        const assistantsById = new Map(
          (await listAssistants()).map((assistant) => [assistant.id, assistant]),
        );
        if (generation !== requestGenerationRef.current) {
          return;
        }

        assistantsByIdRef.current = assistantsById;
        setSearchSessions(
          mapSessionMetadataList(normalized.items, new Map(), assistantsById),
        );
        setSearchCursor(normalized.nextCursor);
        setSearchHasMore(Boolean(normalized.nextCursor));
      } catch (error) {
        if (generation !== requestGenerationRef.current) {
          return;
        }
        logger.error('Failed to search sessions', error);
        setSearchSessions([]);
        setSearchCursor(undefined);
        setSearchHasMore(false);
      } finally {
        if (generation === requestGenerationRef.current) {
          setIsSearchLoading(false);
        }
      }
    })();
  }, [
    trimmedDebounced,
    bookmarkedOnly,
    statusFilter,
    refreshToken,
    resetSearchLoadingMore,
  ]);

  const loadMoreSearch = useCallback(async () => {
    const cursor = searchCursorRef.current;
    const query = activeSearchQuery;
    if (!cursor || !query || isSearchLoadingMoreRef.current) {
      return;
    }

    isSearchLoadingMoreRef.current = true;
    setIsSearchLoadingMore(true);
    const generation = requestGenerationRef.current;

    try {
      const response = await safeInvoke<AgentSessionListResponse>(
        'agent_list_sessions',
        {
          request: buildSearchRequest({
            search: query,
            cursor,
            bookmarkedOnly,
            statusFilter,
          }),
        },
      );
      if (generation !== requestGenerationRef.current) {
        return;
      }

      const normalized = normalizeSessionListResponse(response);
      let assistantsById = assistantsByIdRef.current;
      if (assistantsById.size === 0) {
        assistantsById = new Map(
          (await listAssistants()).map((assistant) => [assistant.id, assistant]),
        );
        if (generation !== requestGenerationRef.current) {
          return;
        }
        assistantsByIdRef.current = assistantsById;
      }

      const incoming = mapSessionMetadataList(
        normalized.items,
        new Map(),
        assistantsById,
      );

      setSearchSessions((previous) =>
        sortSessionsByLatestActivity(
          dedupeSessionsById([...previous, ...incoming]),
        ),
      );
      setSearchCursor(normalized.nextCursor);
      setSearchHasMore(Boolean(normalized.nextCursor));
    } catch (error) {
      logger.error('Failed to load more search results', error);
      throw error;
    } finally {
      isSearchLoadingMoreRef.current = false;
      setIsSearchLoadingMore(false);
    }
  }, [activeSearchQuery, bookmarkedOnly, statusFilter]);

  const ensureSearchChildrenLoaded = useCallback(
    async (parentSessionId: string) => {
      if (!isServerSearchActive) {
        return;
      }
      if (childrenLoadCompletedRef.current.has(parentSessionId)) {
        return;
      }

      const existing = childrenLoadInflightRef.current.get(parentSessionId);
      if (existing) {
        return existing;
      }

      const loadPromise = (async () => {
        try {
          const childMetadata = await safeInvoke<AgentSessionMetadata[]>(
            'agent_get_child_sessions',
            { sessionId: parentSessionId },
          );

          if (Array.isArray(childMetadata) && childMetadata.length > 0) {
            let assistantsById = assistantsByIdRef.current;
            if (assistantsById.size === 0) {
              assistantsById = new Map(
                (await listAssistants()).map((assistant) => [
                  assistant.id,
                  assistant,
                ]),
              );
              assistantsByIdRef.current = assistantsById;
            }

            setSearchSessions((previous) => {
              const pendingApprovalCounts = new Map(
                previous.map((session) => [
                  session.id,
                  session.pendingApprovalCount ?? 0,
                ]),
              );
              const incoming = mapSessionMetadataList(
                childMetadata,
                pendingApprovalCounts,
                assistantsById,
              );
              return sortSessionsByLatestActivity(
                dedupeSessionsById([...previous, ...incoming]),
              );
            });
          }

          childrenLoadCompletedRef.current.add(parentSessionId);
        } catch (error) {
          logger.error('Failed to load search child sessions', {
            parentSessionId,
            error,
          });
          throw error;
        } finally {
          childrenLoadInflightRef.current.delete(parentSessionId);
        }
      })();

      childrenLoadInflightRef.current.set(parentSessionId, loadPromise);
      return loadPromise;
    },
    [isServerSearchActive],
  );

  const removeSearchSessionTree = useCallback((sessionId: string) => {
    setSearchSessions((previous) => {
      const idsToRemove = collectSubtreeIds(previous, sessionId);
      return previous.filter((session) => !idsToRemove.has(session.id));
    });
  }, []);

  const removeSearchSessionOnly = useCallback((sessionId: string) => {
    setSearchSessions((previous) =>
      previous
        .filter((session) => session.id !== sessionId)
        .map((session) =>
          session.parentSessionId === sessionId
            ? { ...session, parentSessionId: undefined }
            : session,
        ),
    );
  }, []);

  const setSearchSessionBookmarked = useCallback(
    (sessionId: string, bookmarked: boolean) => {
      setSearchSessions((previous) =>
        previous.map((session) =>
          session.id === sessionId
            ? { ...session, isBookmarked: bookmarked }
            : session,
        ),
      );
    },
    [],
  );

  const idleMutations = {
    ensureSearchChildrenLoaded,
    removeSearchSessionTree,
    removeSearchSessionOnly,
    setSearchSessionBookmarked,
  };

  // Debouncing into the first server search: keep browse unfiltered and show loading
  // instead of client-filtering the sidebar page (which looks like "missing results").
  if (wantsServerSearch && !isServerSearchActive) {
    return {
      displaySessions: browseSessions,
      hasMoreSessions: false,
      isLoading: true,
      isLoadingMore: false,
      clientSearchQuery: '',
      isServerSearchActive: false,
      loadMore: onBrowseLoadMore,
      ...idleMutations,
    };
  }

  if (!isServerSearchActive) {
    return {
      displaySessions: browseSessions,
      hasMoreSessions: browseHasMore,
      isLoading: browseLoading,
      isLoadingMore: browseLoadingMore,
      clientSearchQuery: searchQuery,
      isServerSearchActive: false,
      loadMore: onBrowseLoadMore,
      ...idleMutations,
    };
  }

  return {
    displaySessions: searchSessions,
    hasMoreSessions: searchHasMore,
    isLoading: isDebouncing || isSearchLoading,
    isLoadingMore: isSearchLoadingMore,
    clientSearchQuery: '',
    isServerSearchActive: true,
    loadMore: loadMoreSearch,
    ...idleMutations,
  };
}
