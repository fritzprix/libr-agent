import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { RefreshCw } from 'lucide-react';
import {
  buildDescendantCounts,
  buildDescendantStatusCounts,
  type SessionStatus,
  type SessionStatusCounts,
} from '@/lib/session-utils';
import { sortSessionsByLatestActivity } from '@/lib/session-metadata';
import type { AgentSession } from '@/models/agent';
import { SessionCard } from './SessionCard';

// Extracted modules
import {
  HISTORY_CONTENT_RAIL_CLASS,
  HISTORY_SECTION_CLASS,
  BOOKMARK_PREVIEW_LIMIT,
  TREE_INDENT_PX,
  MAX_TREE_INDENT_LEVEL,
  type SessionSortKey,
  type SessionSortDirection,
} from './session-history-utils';
import { computeSessionTree, type SessionHistoryRow } from './session-tree';
import { useInfiniteScroll } from './use-session-scroll';
import { useKnownDirectChildCounts } from './use-known-direct-child-counts';
import { BookmarkedSessionsSection } from './session-history-panel/BookmarkedSessionsSection';
import { SessionHistoryControls } from './session-history-panel/SessionHistoryControls';
import { SessionHistoryEmptyState } from './session-history-panel/SessionHistoryEmptyState';
import { SessionHistoryFocusBar } from './session-history-panel/SessionHistoryFocusBar';
import { SessionHistoryHeader } from './session-history-panel/SessionHistoryHeader';

interface SessionHistoryPanelProps {
  sessions: AgentSession[];
  isLoading: boolean;
  hasMoreSessions: boolean;
  isLoadingMoreSessions: boolean;
  activeStatusFilter: 'all' | SessionStatus;
  searchQuery: string;
  /**
   * Text used for in-memory filtering. Defaults to `searchQuery`.
   * Pass `''` when the session list is already server-filtered.
   */
  clientSearchQuery?: string;
  onActiveStatusFilterChange: (value: 'all' | SessionStatus) => void;
  onSearchQueryChange: (value: string) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
  onEnsureChildrenLoaded?: (sessionId: string) => void | Promise<void>;
  loadingChildrenParentIds?: ReadonlySet<string>;
  onResume: (sessionId: string) => void;
  onDelete: (sessionId: string) => void;
  onDeleteOnly?: (sessionId: string) => void;
  onToggleBookmark?: (sessionId: string) => void;
  heading?: string;
  description?: string;
  searchPlaceholder?: string;
  emptyStateTitle?: string;
  emptyStateSubtitle?: string;
  initialSortKey?: SessionSortKey;
  initialSortDirection?: SessionSortDirection;
  showBookmarkedOnly?: boolean;
  onShowBookmarkedOnlyChange?: (value: boolean) => void;
}

export function SessionHistoryPanel({
  sessions,
  isLoading,
  hasMoreSessions,
  isLoadingMoreSessions,
  activeStatusFilter,
  searchQuery,
  clientSearchQuery,
  onActiveStatusFilterChange,
  onSearchQueryChange,
  onRefresh,
  onLoadMore,
  onEnsureChildrenLoaded,
  loadingChildrenParentIds,
  onResume,
  onDelete,
  onDeleteOnly,
  onToggleBookmark,
  heading,
  description,
  searchPlaceholder,
  emptyStateTitle,
  emptyStateSubtitle,
  initialSortKey = 'updatedAt',
  initialSortDirection = 'desc',
  showBookmarkedOnly: controlledShowBookmarkedOnly,
  onShowBookmarkedOnlyChange,
}: SessionHistoryPanelProps) {
  const { t } = useTranslation('common');
  const rootRef = useRef<HTMLDivElement | null>(null);
  const loadMoreSentinelRef = useRef<HTMLDivElement | null>(null);
  const historyControlsRef = useRef<HTMLDivElement | null>(null);
  const [selectedLineageId, setSelectedLineageId] = useState<string | null>(
    null,
  );
  const [localShowBookmarkedOnly, setLocalShowBookmarkedOnly] = useState(false);
  const showBookmarkedOnly =
    controlledShowBookmarkedOnly !== undefined
      ? controlledShowBookmarkedOnly
      : localShowBookmarkedOnly;

  const setShowBookmarkedOnly = useCallback(
    (value: boolean | ((prev: boolean) => boolean)) => {
      if (controlledShowBookmarkedOnly !== undefined) {
        const nextValue =
          typeof value === 'function'
            ? value(controlledShowBookmarkedOnly)
            : value;
        onShowBookmarkedOnlyChange?.(nextValue);
      } else {
        setLocalShowBookmarkedOnly(value);
      }
    },
    [controlledShowBookmarkedOnly, onShowBookmarkedOnlyChange],
  );
  const [activeSortKey, setActiveSortKey] =
    useState<SessionSortKey>(initialSortKey);
  const [activeSortDirection, setActiveSortDirection] =
    useState<SessionSortDirection>(initialSortDirection);
  const [manuallyExpandedSessionIds, setManuallyExpandedSessionIds] = useState<
    Set<string>
  >(() => new Set());
  const [collapsedAutoExpandedSessionIds, setCollapsedAutoExpandedSessionIds] =
    useState<Set<string>>(() => new Set());

  const defaultHeading =
    heading ?? t('sessionHistory.defaultHeading', 'Recent Sessions');
  const defaultDescription =
    description ??
    t('sessionHistory.defaultDescription', 'Resume previous agent sessions');
  const defaultSearchPlaceholder =
    searchPlaceholder ??
    t('sessionHistory.searchPlaceholder', 'Search sessions by name or ID...');
  const defaultEmptyTitle =
    emptyStateTitle ??
    t('sessionHistory.defaultEmptyTitle', 'No previous sessions');
  const defaultEmptySubtitle =
    emptyStateSubtitle ??
    t(
      'sessionHistory.defaultEmptySubtitle',
      'Start a conversation to create your first session',
    );

  const textFilterQuery =
    clientSearchQuery !== undefined ? clientSearchQuery : searchQuery;
  const deferredSessions = useDeferredValue(sessions);
  const deferredSearchQuery = useDeferredValue(textFilterQuery);
  const knownDirectChildCountByParentId = useKnownDirectChildCounts(
    sessions,
    hasMoreSessions,
  );
  const isPending =
    textFilterQuery !== deferredSearchQuery || sessions !== deferredSessions;

  const filtersActive =
    activeStatusFilter !== 'all' ||
    searchQuery.trim().length > 0 ||
    deferredSearchQuery.trim().length > 0 ||
    showBookmarkedOnly;

  useEffect(() => {
    if (!selectedLineageId) {
      return;
    }

    const stillExists = sessions.some(
      (session) => session.lineageId === selectedLineageId,
    );
    if (!stillExists) {
      setSelectedLineageId(null);
    }
  }, [selectedLineageId, sessions]);

  useEffect(() => {
    setCollapsedAutoExpandedSessionIds(new Set());
  }, [
    activeSortDirection,
    activeSortKey,
    activeStatusFilter,
    deferredSearchQuery,
    selectedLineageId,
    showBookmarkedOnly,
  ]);

  const bookmarkedSessions = useMemo(
    () =>
      sortSessionsByLatestActivity(
        deferredSessions.filter((session) => session.isBookmarked === true),
      ),
    [deferredSessions],
  );
  const featuredBookmarkedSessions = bookmarkedSessions.slice(
    0,
    BOOKMARK_PREVIEW_LIMIT,
  );
  const remainingBookmarkedCount = Math.max(
    bookmarkedSessions.length - featuredBookmarkedSessions.length,
    0,
  );

  const descendantCounts = useMemo(
    () => buildDescendantCounts(sessions),
    [sessions],
  );

  const previousDescendantStatusCountsRef = useRef<
    Map<string, SessionStatusCounts>
  >(new Map());

  const descendantStatusCounts = useMemo(() => {
    const nextCounts = buildDescendantStatusCounts(
      sessions,
      previousDescendantStatusCountsRef.current,
    );
    previousDescendantStatusCountsRef.current = nextCounts;
    return nextCounts;
  }, [sessions]);

  const {
    autoExpandedAncestorIds,
    baseSessions,
    displayRows,
    matchedSessionCount,
    statusCounts,
  } = useMemo(
    () =>
      computeSessionTree({
        deferredSessions,
        selectedLineageId,
        showBookmarkedOnly,
        activeStatusFilter,
        deferredSearchQuery,
        activeSortKey,
        activeSortDirection,
        manuallyExpandedSessionIds,
        collapsedAutoExpandedSessionIds,
        descendantStatusCounts,
        knownDirectChildCountByParentId,
        t,
      }),
    [
      deferredSessions,
      selectedLineageId,
      showBookmarkedOnly,
      activeStatusFilter,
      deferredSearchQuery,
      activeSortKey,
      activeSortDirection,
      manuallyExpandedSessionIds,
      collapsedAutoExpandedSessionIds,
      descendantStatusCounts,
      knownDirectChildCountByParentId,
      t,
    ],
  );

  // Custom hook for infinite scroll logic
  useInfiniteScroll({
    rootRef,
    loadMoreSentinelRef,
    hasMoreSessions,
    isLoadingMoreSessions,
    onLoadMore,
    displayRowsLength: displayRows.length,
  });

  const expandStateRef = useRef({
    autoExpandedAncestorIds,
    manuallyExpandedSessionIds,
    collapsedAutoExpandedSessionIds,
    onEnsureChildrenLoaded,
  });
  expandStateRef.current = {
    autoExpandedAncestorIds,
    manuallyExpandedSessionIds,
    collapsedAutoExpandedSessionIds,
    onEnsureChildrenLoaded,
  };

  const handleToggleExpand = useCallback((sessionId: string) => {
    const {
      autoExpandedAncestorIds: autoExpanded,
      manuallyExpandedSessionIds: manuallyExpanded,
      collapsedAutoExpandedSessionIds: collapsedAuto,
      onEnsureChildrenLoaded: ensureChildrenLoaded,
    } = expandStateRef.current;

    const isAutoExpanded = autoExpanded.has(sessionId);
    const isExpanded =
      manuallyExpanded.has(sessionId) ||
      (isAutoExpanded && !collapsedAuto.has(sessionId));

    const willExpand = !isExpanded;

    setManuallyExpandedSessionIds((prev) => {
      const next = new Set(prev);
      if (isExpanded) {
        next.delete(sessionId);
      } else if (!isAutoExpanded) {
        next.add(sessionId);
      }
      return next;
    });

    if (isAutoExpanded) {
      setCollapsedAutoExpandedSessionIds((prev) => {
        const next = new Set(prev);
        if (isExpanded) {
          next.add(sessionId);
        } else {
          next.delete(sessionId);
        }
        return next;
      });
    }

    if (willExpand) {
      void Promise.resolve(ensureChildrenLoaded?.(sessionId));
    }
  }, []);

  // Search/filter can auto-expand ancestors of matched rows. Those parents may
  // still lack children in the paginated list, so prefetch direct children for
  // every currently expanded auto-ancestor (deduped inside ensureChildrenLoaded).
  useEffect(() => {
    if (!onEnsureChildrenLoaded) {
      return;
    }

    autoExpandedAncestorIds.forEach((sessionId) => {
      if (collapsedAutoExpandedSessionIds.has(sessionId)) {
        return;
      }
      void Promise.resolve(onEnsureChildrenLoaded(sessionId));
    });
  }, [
    autoExpandedAncestorIds,
    collapsedAutoExpandedSessionIds,
    onEnsureChildrenLoaded,
  ]);

  const handleBrowseBookmarkedHistory = useCallback(() => {
    setShowBookmarkedOnly(true);
    window.requestAnimationFrame(() => {
      historyControlsRef.current?.scrollIntoView?.({
        behavior: 'smooth',
        block: 'start',
      });
    });
  }, []);

  const handleEndReached = useCallback(() => {
    if (!hasMoreSessions || isLoadingMoreSessions) {
      return;
    }

    onLoadMore();
  }, [hasMoreSessions, isLoadingMoreSessions, onLoadMore]);

  const handleLineageSelect = useCallback((lineageId: string) => {
    setSelectedLineageId((prev) => (prev === lineageId ? null : lineageId));
  }, []);

  const renderSessionRow = useCallback(
    ({
      session,
      nestingLevel,
      lineageHint,
      hasExpandableChildren,
      isExpanded,
      hiddenChildrenCount,
      unloadedChildrenCount,
      descendantStatusCounts: rowDescendantStatusCounts,
    }: SessionHistoryRow) => {
      const indentationPx =
        nestingLevel > 0
          ? Math.min(nestingLevel, MAX_TREE_INDENT_LEVEL) * TREE_INDENT_PX
          : 0;

      return (
        <div
          key={session.id}
          role="listitem"
          style={
            indentationPx > 0
              ? { paddingLeft: `${indentationPx}px` }
              : undefined
          }
        >
          <div
            className={cn(
              'pb-4',
              nestingLevel > 0 &&
                'border-l border-border/50 pl-3 dark:border-border/60',
            )}
          >
            <SessionCard
              session={session}
              onResume={onResume}
              onDelete={onDelete}
              onDeleteOnly={onDeleteOnly}
              onToggleBookmark={onToggleBookmark}
              lineageHint={lineageHint}
              selectedLineageId={selectedLineageId}
              descendantCount={descendantCounts.get(session.id) ?? 0}
              descendantStatusCounts={rowDescendantStatusCounts}
              hasExpandableChildren={hasExpandableChildren}
              isExpanded={isExpanded}
              hiddenChildrenCount={hiddenChildrenCount}
              unloadedChildrenCount={unloadedChildrenCount}
              isLoadingChildren={loadingChildrenParentIds?.has(session.id)}
              onToggleExpand={handleToggleExpand}
              onLineageSelect={handleLineageSelect}
            />
          </div>
        </div>
      );
    },
    [
      descendantCounts,
      handleLineageSelect,
      handleToggleExpand,
      loadingChildrenParentIds,
      onDelete,
      onDeleteOnly,
      onResume,
      onToggleBookmark,
      selectedLineageId,
    ],
  );

  return (
    <div ref={rootRef} className="flex min-h-full flex-col bg-background p-6">
      <SessionHistoryHeader
        heading={defaultHeading}
        description={defaultDescription}
        showBookmarkedOnly={showBookmarkedOnly}
        baseSessionCount={baseSessions.length}
        visibleRowCount={displayRows.length}
        hasMoreSessions={hasMoreSessions}
        filtersActive={filtersActive}
        matchedSessionCount={matchedSessionCount}
        isLoading={isLoading}
        onRefresh={onRefresh}
      />

      {!selectedLineageId &&
        !showBookmarkedOnly &&
        bookmarkedSessions.length > 0 && (
          <BookmarkedSessionsSection
            totalCount={bookmarkedSessions.length}
            featuredSessions={featuredBookmarkedSessions}
            remainingCount={remainingBookmarkedCount}
            onBrowseAll={handleBrowseBookmarkedHistory}
            onResume={onResume}
            onToggleBookmark={onToggleBookmark}
          />
        )}

      <div
        ref={historyControlsRef}
        className={cn(
          HISTORY_CONTENT_RAIL_CLASS,
          HISTORY_SECTION_CLASS,
          'mb-4 flex flex-col gap-3',
        )}
      >
        <SessionHistoryControls
          searchPlaceholder={defaultSearchPlaceholder}
          searchQuery={searchQuery}
          onSearchQueryChange={onSearchQueryChange}
          showBookmarkedOnly={showBookmarkedOnly}
          onToggleShowBookmarkedOnly={() =>
            setShowBookmarkedOnly((previousState) => !previousState)
          }
          bookmarkedCount={bookmarkedSessions.length}
          activeSortKey={activeSortKey}
          onActiveSortKeyChange={setActiveSortKey}
          activeSortDirection={activeSortDirection}
          onToggleSortDirection={() =>
            setActiveSortDirection((previousState) =>
              previousState === 'asc' ? 'desc' : 'asc',
            )
          }
          activeStatusFilter={activeStatusFilter}
          onActiveStatusFilterChange={onActiveStatusFilterChange}
          statusCounts={statusCounts}
        />
      </div>

      {(selectedLineageId || showBookmarkedOnly) && (
        <SessionHistoryFocusBar
          selectedLineageId={selectedLineageId}
          showBookmarkedOnly={showBookmarkedOnly}
          onClearLineage={() => setSelectedLineageId(null)}
          onClearBookmarkFilter={() => setShowBookmarkedOnly(false)}
        />
      )}

      <div
        className={cn(
          HISTORY_CONTENT_RAIL_CLASS,
          'mt-6 min-h-[18rem] transition-opacity duration-200',
          isPending ? 'opacity-50' : 'opacity-100',
        )}
        aria-busy={isPending}
      >
        {isPending && (
          <div className="sr-only" aria-live="polite">
            {t('sessionHistory.filteringStatus', 'Filtering sessions...')}
          </div>
        )}
        {isLoading && sessions.length === 0 ? (
          <div className="flex h-full items-center justify-center">
            <div className="text-center text-muted-foreground">
              <RefreshCw className="mx-auto mb-2 h-8 w-8 animate-spin" />
              <p className="text-sm">
                {t('sessionHistory.loading', 'Loading sessions...')}
              </p>
            </div>
          </div>
        ) : displayRows.length === 0 ? (
          <SessionHistoryEmptyState
            selectedLineageId={selectedLineageId}
            searchQuery={searchQuery}
            showBookmarkedOnly={showBookmarkedOnly}
            emptyTitle={defaultEmptyTitle}
            emptySubtitle={defaultEmptySubtitle}
            onClearLineage={() => setSelectedLineageId(null)}
            onClearSearch={() => onSearchQueryChange('')}
            onClearBookmarkFilter={() => setShowBookmarkedOnly(false)}
          />
        ) : (
          <div
            className="w-full pb-4"
            role="list"
            aria-labelledby="session-heading"
          >
            {displayRows.map(renderSessionRow)}
            <div
              ref={loadMoreSentinelRef}
              data-testid="session-history-load-more-sentinel"
              aria-hidden="true"
              className="h-px w-full"
            />
            {hasMoreSessions ? (
              <div className="flex justify-center py-4">
                <Button
                  variant="outline"
                  onClick={handleEndReached}
                  disabled={isLoadingMoreSessions}
                >
                  <RefreshCw
                    className={cn(
                      'mr-2 h-4 w-4',
                      isLoadingMoreSessions && 'animate-spin',
                    )}
                  />
                  {isLoadingMoreSessions
                    ? t('sessionHistory.loadingMore', 'Loading more...')
                    : t('sessionHistory.loadMore', 'Load more')}
                </Button>
              </div>
            ) : null}
          </div>
        )}
      </div>
    </div>
  );
}
