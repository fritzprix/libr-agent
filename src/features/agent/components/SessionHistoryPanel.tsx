import {
  useCallback,
  useDeferredValue,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { cn } from '@/lib/utils';
import {
  RefreshCw,
  Search,
  History,
  X,
  BookmarkCheck,
  Clock3,
  StarOff,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import {
  buildChildrenMap,
  buildDescendantCounts,
  buildDescendantStatusCounts,
  filterSessions,
  type SessionStatus,
  type SessionStatusCounts,
} from '@/lib/session-utils';
import { formatRelativeTime } from '@/lib/date-utils';
import {
  getLatestSessionActivityTimestamp,
  sortSessionsByLatestActivity,
} from '@/lib/session-metadata';
import type { AgentSession } from '@/models/agent';
import { SessionCard } from './SessionCard';

interface SessionHistoryPanelProps {
  sessions: AgentSession[];
  isLoading: boolean;
  hasMoreSessions: boolean;
  isLoadingMoreSessions: boolean;
  activeStatusFilter: 'all' | SessionStatus;
  searchQuery: string;
  onActiveStatusFilterChange: (value: 'all' | SessionStatus) => void;
  onSearchQueryChange: (value: string) => void;
  onRefresh: () => void;
  onLoadMore: () => void;
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
}

interface SessionHistoryRow {
  session: AgentSession;
  nestingLevel: number;
  lineageHint?: string;
  hasExpandableChildren: boolean;
  isExpanded: boolean;
  descendantStatusCounts?: SessionStatusCounts;
}

type SessionHistoryTranslate = ReturnType<typeof useTranslation>['t'];
type SessionSortKey = 'updatedAt' | 'createdAt' | 'name';
type SessionSortDirection = 'asc' | 'desc';

function findScrollParent(element: HTMLElement | null): HTMLElement | null {
  let current = element?.parentElement ?? null;

  while (current) {
    const { overflowY } = window.getComputedStyle(current);
    if (overflowY === 'auto' || overflowY === 'scroll') {
      return current;
    }
    current = current.parentElement;
  }

  return null;
}

interface BookmarkedSessionTileProps {
  session: AgentSession;
  onResume: (sessionId: string) => void;
  onToggleBookmark?: (sessionId: string) => void;
  t: SessionHistoryTranslate;
}

const HISTORY_CONTENT_RAIL_CLASS = 'mx-auto w-full max-w-4xl';
const HISTORY_SECTION_CLASS =
  'rounded-xl border bg-card/80 p-4 shadow-sm shadow-black/5';
const BOOKMARK_PREVIEW_LIMIT = 6;
const sessionSortValues: SessionSortKey[] = ['updatedAt', 'createdAt', 'name'];

function getSessionDisplayName(
  session: AgentSession,
  t: SessionHistoryTranslate,
): string {
  return (
    session.name ||
    t('sessionHistory.card.fallbackName', 'Session {{id}}', {
      id: session.id.slice(0, 8),
    })
  );
}

function isSessionSortKey(value: string): value is SessionSortKey {
  return sessionSortValues.includes(value as SessionSortKey);
}

function getSessionSortTimestamp(
  session: AgentSession,
  sortKey: Extract<SessionSortKey, 'updatedAt' | 'createdAt'>,
): number {
  if (sortKey === 'updatedAt') {
    return getLatestSessionActivityTimestamp(session);
  }

  return session.createdAt.getTime();
}

function compareSessionsBySort(
  left: AgentSession,
  right: AgentSession,
  sortKey: SessionSortKey,
  sortDirection: SessionSortDirection,
  t: SessionHistoryTranslate,
): number {
  let comparison = 0;

  if (sortKey === 'name') {
    comparison = getSessionDisplayName(left, t).localeCompare(
      getSessionDisplayName(right, t),
      undefined,
      {
        numeric: true,
        sensitivity: 'base',
      },
    );
  } else {
    comparison =
      getSessionSortTimestamp(left, sortKey) -
      getSessionSortTimestamp(right, sortKey);
  }

  if (comparison === 0) {
    return 0;
  }

  return sortDirection === 'asc' ? comparison : -comparison;
}

function compareSessionsByLatestActivityDesc(
  left: AgentSession,
  right: AgentSession,
): number {
  return (
    getSessionSortTimestamp(right, 'updatedAt') -
    getSessionSortTimestamp(left, 'updatedAt')
  );
}

function BookmarkedSessionRow({
  session,
  onResume,
  onToggleBookmark,
  t,
}: BookmarkedSessionTileProps) {
  const shortcutLabel = getSessionDisplayName(session, t);
  const latestActivity =
    formatRelativeTime(
      new Date(getLatestSessionActivityTimestamp(session)),
      new Date(),
    ) || t('sessionHistory.card.justNow', 'just now');
  const secondaryLabel =
    session.assistant?.name ||
    (session.provider && session.model
      ? `${session.provider}/${session.model}`
      : t(
          'sessionHistory.bookmarkedSection.defaultMeta',
          'Saved for quick access',
        ));

  return (
    <div className="flex items-center gap-2 rounded-lg border bg-background/80 px-3 py-2 shadow-sm shadow-black/5">
      <Button
        type="button"
        variant="ghost"
        className="-my-1 h-auto min-w-0 flex-1 justify-start px-2 py-1"
        onClick={() => onResume(session.id)}
        aria-label={t(
          'sessionHistory.bookmarkedSection.resumeAria',
          'Open bookmarked session {{name}}',
          { name: shortcutLabel },
        )}
      >
        <div className="min-w-0 flex-1 text-left">
          <div className="truncate text-sm font-semibold leading-5 text-foreground">
            {shortcutLabel}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <span className="truncate">{secondaryLabel}</span>
            <span aria-hidden="true">•</span>
            <Clock3 className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">
              {t('sessionHistory.bookmarkedSection.lastUsed', 'Used {{time}}', {
                time: latestActivity,
              })}
            </span>
          </div>
        </div>
      </Button>
      <Button
        type="button"
        size="icon"
        variant="ghost"
        className="h-8 w-8 shrink-0"
        onClick={() => onToggleBookmark?.(session.id)}
        aria-label={t(
          'sessionHistory.actions.unbookmarkAria',
          'Remove bookmark',
        )}
      >
        <StarOff className="h-4 w-4" aria-hidden="true" />
      </Button>
    </div>
  );
}

const statusPriority: Record<string, number> = {
  busy: 1,
  idle: 2,
  paused: 3,
  error: 4,
};

const statusFilterValues: Array<'all' | SessionStatus> = [
  'all',
  'busy',
  'idle',
  'paused',
  'error',
];

function isSessionStatusFilterValue(
  value: string,
): value is 'all' | SessionStatus {
  return statusFilterValues.includes(value as 'all' | SessionStatus);
}

export function SessionHistoryPanel({
  sessions,
  isLoading,
  hasMoreSessions,
  isLoadingMoreSessions,
  activeStatusFilter,
  searchQuery,
  onActiveStatusFilterChange,
  onSearchQueryChange,
  onRefresh,
  onLoadMore,
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
}: SessionHistoryPanelProps) {
  const { t } = useTranslation('common');
  const rootRef = useRef<HTMLDivElement | null>(null);
  const scrollParentRef = useRef<HTMLElement | null>(null);
  const loadMoreSentinelRef = useRef<HTMLDivElement | null>(null);
  const historyControlsRef = useRef<HTMLDivElement | null>(null);
  const [selectedLineageId, setSelectedLineageId] = useState<string | null>(
    null,
  );
  const [showBookmarkedOnly, setShowBookmarkedOnly] = useState(false);
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

  const deferredSessions = useDeferredValue(sessions);
  const deferredSearchQuery = useDeferredValue(searchQuery);
  const isPending =
    searchQuery !== deferredSearchQuery || sessions !== deferredSessions;

  const filtersActive =
    activeStatusFilter !== 'all' ||
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

  useLayoutEffect(() => {
    scrollParentRef.current = findScrollParent(rootRef.current);
  }, []);

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

  const descendantStatusCounts = useMemo(
    () => buildDescendantStatusCounts(sessions),
    [sessions],
  );

  const {
    autoExpandedAncestorIds,
    baseSessions,
    displayRows,
    matchedSessionCount,
    statusCounts,
  } = useMemo(() => {
    const lineageSessions = selectedLineageId
      ? deferredSessions.filter(
          (session) => session.lineageId === selectedLineageId,
        )
      : deferredSessions;
    const bookmarkedScopeSessions = showBookmarkedOnly
      ? lineageSessions.filter((session) => session.isBookmarked === true)
      : lineageSessions;
    const nextStatusCounts = {
      all: bookmarkedScopeSessions.length,
      busy: 0,
      idle: 0,
      paused: 0,
      error: 0,
    };

    bookmarkedScopeSessions.forEach((session) => {
      if (
        Object.prototype.hasOwnProperty.call(nextStatusCounts, session.status)
      ) {
        nextStatusCounts[session.status as keyof typeof nextStatusCounts]++;
      }
    });

    let filteredSessions = bookmarkedScopeSessions;

    if (activeStatusFilter !== 'all') {
      filteredSessions = filteredSessions.filter(
        (session) => session.status === activeStatusFilter,
      );
    }

    const matchedSessions = [
      ...filterSessions(filteredSessions, deferredSearchQuery),
    ].sort((a, b) => {
      const sortDiff = compareSessionsBySort(
        a,
        b,
        activeSortKey,
        activeSortDirection,
        t,
      );
      if (sortDiff !== 0) {
        return sortDiff;
      }

      const statusDiff =
        (statusPriority[a.status] ?? 999) - (statusPriority[b.status] ?? 999);
      if (statusDiff !== 0) {
        return statusDiff;
      }

      const latestActivityDiff = compareSessionsByLatestActivityDesc(a, b);
      if (latestActivityDiff !== 0) {
        return latestActivityDiff;
      }

      return b.createdAt.getTime() - a.createdAt.getTime();
    });

    // Bolt: Eliminate .map() array allocation, map manually
    const sessionById = new Map<string, AgentSession>();
    for (let i = 0; i < lineageSessions.length; i++) {
      sessionById.set(lineageSessions[i].id, lineageSessions[i]);
    }
    const childrenByParent = buildChildrenMap(lineageSessions);
    const visibleIds = new Set<string>();
    const nextAutoExpandedAncestorIds = new Set<string>();

    matchedSessions.forEach((session) => {
      let current: AgentSession | undefined = session;
      while (current) {
        // Bolt: Break early if already visible to prevent O(N * Depth) traversal
        if (visibleIds.has(current.id)) {
          break;
        }

        visibleIds.add(current.id);
        const parent: AgentSession | undefined = current.parentSessionId
          ? sessionById.get(current.parentSessionId)
          : undefined;
        if (parent && filtersActive) {
          nextAutoExpandedAncestorIds.add(parent.id);
        }
        current = parent;
      }
    });

    const effectiveExpandedSessionIds = new Set(manuallyExpandedSessionIds);
    nextAutoExpandedAncestorIds.forEach((sessionId) => {
      if (!collapsedAutoExpandedSessionIds.has(sessionId)) {
        effectiveExpandedSessionIds.add(sessionId);
      }
    });

    // Bolt: Eliminate .map() array allocation, map manually
    const sortIndexById = new Map<string, number>();
    for (let i = 0; i < matchedSessions.length; i++) {
      sortIndexById.set(matchedSessions[i].id, i);
    }
    const orderCache = new Map<string, number>();
    const orderForSession = (session: AgentSession): number => {
      const cachedOrder = orderCache.get(session.id);
      if (cachedOrder !== undefined) {
        return cachedOrder;
      }

      let computedOrder: number;
      const indexOrder = sortIndexById.get(session.id);
      if (indexOrder !== undefined) {
        computedOrder = indexOrder;
      } else {
        const descendants = childrenByParent.get(session.id) || [];
        // Bolt: Replaced .filter().map() with single-pass manual loop
        // to prevent allocating intermediate arrays.
        let minDescendantOrder = Number.MAX_SAFE_INTEGER;
        for (let i = 0; i < descendants.length; i++) {
          const child = descendants[i];
          if (visibleIds.has(child.id)) {
            const childOrder = orderForSession(child);
            if (childOrder < minDescendantOrder) {
              minDescendantOrder = childOrder;
            }
          }
        }
        computedOrder = minDescendantOrder;
      }

      orderCache.set(session.id, computedOrder);
      return computedOrder;
    };

    const sortByCurrentOrder = (a: AgentSession, b: AgentSession) => {
      const orderDiff = orderForSession(a) - orderForSession(b);
      if (orderDiff !== 0) {
        return orderDiff;
      }

      const statusDiff =
        (statusPriority[a.status] ?? 999) - (statusPriority[b.status] ?? 999);
      if (statusDiff !== 0) {
        return statusDiff;
      }

      return b.createdAt.getTime() - a.createdAt.getTime();
    };

    for (const children of childrenByParent.values()) {
      children.sort(sortByCurrentOrder);
    }

    const roots = lineageSessions
      .filter((session) => {
        if (!visibleIds.has(session.id)) {
          return false;
        }

        return (
          !session.parentSessionId || !visibleIds.has(session.parentSessionId)
        );
      })
      .sort(sortByCurrentOrder);

    const rows: SessionHistoryRow[] = [];

    const walk = (session: AgentSession, nestingLevel: number) => {
      const visibleChildren = (childrenByParent.get(session.id) || []).filter(
        (child) => visibleIds.has(child.id),
      );
      const parentName = session.parentSessionId
        ? sessionById.get(session.parentSessionId)?.name ||
          t('sessionHistory.card.fallbackName', 'Session {{id}}', {
            id: session.parentSessionId.slice(0, 8),
          })
        : undefined;
      const hasExpandableChildren = visibleChildren.length > 0;
      const isExpanded = hasExpandableChildren
        ? effectiveExpandedSessionIds.has(session.id)
        : false;

      rows.push({
        session,
        nestingLevel,
        lineageHint: parentName
          ? t('sessionHistory.lineageHint.child', '↳ Child of {{parentName}}', {
              parentName,
            })
          : t('sessionHistory.lineageHint.topLevel', 'Top-level session'),
        hasExpandableChildren,
        isExpanded,
        descendantStatusCounts: descendantStatusCounts.get(session.id),
      });

      if (!isExpanded) {
        return;
      }

      visibleChildren.forEach((child) => {
        walk(child, nestingLevel + 1);
      });
    };

    roots.forEach((root) => {
      walk(root, 0);
    });

    return {
      autoExpandedAncestorIds: nextAutoExpandedAncestorIds,
      baseSessions: bookmarkedScopeSessions,
      displayRows: rows,
      matchedSessionCount: matchedSessions.length,
      statusCounts: nextStatusCounts,
    };
  }, [
    activeStatusFilter,
    activeSortDirection,
    activeSortKey,
    collapsedAutoExpandedSessionIds,
    descendantStatusCounts,
    deferredSearchQuery,
    deferredSessions,
    filtersActive,
    manuallyExpandedSessionIds,
    selectedLineageId,
    showBookmarkedOnly,
    t,
  ]);

  const handleToggleExpand = useCallback(
    (sessionId: string) => {
      const isAutoExpanded = autoExpandedAncestorIds.has(sessionId);
      const isExpanded =
        manuallyExpandedSessionIds.has(sessionId) ||
        (isAutoExpanded && !collapsedAutoExpandedSessionIds.has(sessionId));

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
    },
    [
      autoExpandedAncestorIds,
      collapsedAutoExpandedSessionIds,
      manuallyExpandedSessionIds,
    ],
  );

  const statusFilterLabelByValue: Record<'all' | SessionStatus, string> = {
    all: t('sessionHistory.statusFilter.all', 'All statuses'),
    busy: `${t('sessionHistory.tabs.busy', 'Busy')} (${statusCounts.busy})`,
    idle: `${t('sessionHistory.tabs.idle', 'Idle')} (${statusCounts.idle})`,
    paused: `${t('sessionHistory.tabs.paused', 'Paused')} (${statusCounts.paused})`,
    error: `${t('sessionHistory.tabs.error', 'Error')} (${statusCounts.error})`,
  };
  const sortLabelByValue: Record<SessionSortKey, string> = {
    updatedAt: t('sessionHistory.sort.updatedAt', 'Recent activity'),
    createdAt: t('sessionHistory.sort.createdAt', 'Created'),
    name: t('sessionHistory.sort.name', 'Name'),
  };
  const handleBrowseBookmarkedHistory = useCallback(() => {
    setShowBookmarkedOnly(true);
    window.requestAnimationFrame(() => {
      historyControlsRef.current?.scrollIntoView?.({
        behavior: 'smooth',
        block: 'start',
      });
    });
  }, []);
  const sortDirectionToggleLabel =
    activeSortDirection === 'asc'
      ? t('sessionHistory.sort.descending', 'Sort descending')
      : t('sessionHistory.sort.ascending', 'Sort ascending');

  const handleEndReached = useCallback(() => {
    if (!hasMoreSessions || isLoadingMoreSessions) {
      return;
    }

    onLoadMore();
  }, [hasMoreSessions, isLoadingMoreSessions, onLoadMore]);

  const checkShouldLoadMore = useCallback(() => {
    if (!hasMoreSessions || isLoadingMoreSessions) {
      return;
    }

    const scrollParent = scrollParentRef.current;
    const sentinel = loadMoreSentinelRef.current;
    if (!scrollParent || !sentinel) {
      return;
    }

    const sentinelBottom = sentinel.getBoundingClientRect().bottom;
    const scrollParentBottom = scrollParent.getBoundingClientRect().bottom;

    if (sentinelBottom - scrollParentBottom <= 240) {
      onLoadMore();
    }
  }, [hasMoreSessions, isLoadingMoreSessions, onLoadMore]);

  useEffect(() => {
    const scrollParent = scrollParentRef.current;
    if (!scrollParent) {
      return;
    }

    let frameId: number | null = null;
    const scheduleCheck = () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        checkShouldLoadMore();
      });
    };

    scrollParent.addEventListener('scroll', scheduleCheck, { passive: true });
    window.addEventListener('resize', scheduleCheck);
    scheduleCheck();

    return () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      scrollParent.removeEventListener('scroll', scheduleCheck);
      window.removeEventListener('resize', scheduleCheck);
    };
  }, [checkShouldLoadMore]);

  useEffect(() => {
    const frameId = window.requestAnimationFrame(() => {
      checkShouldLoadMore();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [checkShouldLoadMore, displayRows.length]);

  const renderSessionRow = useCallback(
    ({
      session,
      nestingLevel,
      lineageHint,
      hasExpandableChildren,
      isExpanded,
      descendantStatusCounts: rowDescendantStatusCounts,
    }: SessionHistoryRow) => (
      <div key={session.id} role="listitem">
        <div className="pb-4">
          <SessionCard
            session={session}
            onResume={onResume}
            onDelete={onDelete}
            onDeleteOnly={onDeleteOnly}
            onToggleBookmark={onToggleBookmark}
            nestingLevel={nestingLevel}
            lineageHint={lineageHint}
            selectedLineageId={selectedLineageId}
            descendantCount={descendantCounts.get(session.id) ?? 0}
            descendantStatusCounts={rowDescendantStatusCounts}
            hasExpandableChildren={hasExpandableChildren}
            isExpanded={isExpanded}
            onToggleExpand={handleToggleExpand}
            onLineageSelect={(lineageId) =>
              setSelectedLineageId((prev) =>
                prev === lineageId ? null : lineageId,
              )
            }
          />
        </div>
      </div>
    ),
    [
      descendantCounts,
      handleToggleExpand,
      onDelete,
      onDeleteOnly,
      onResume,
      onToggleBookmark,
      selectedLineageId,
    ],
  );

  return (
    <div ref={rootRef} className="flex min-h-full flex-col bg-background p-6">
      <div
        className={cn(
          HISTORY_CONTENT_RAIL_CLASS,
          'mb-8 flex items-center justify-between gap-3',
        )}
      >
        <div className="flex items-center gap-4">
          <div className="rounded-xl bg-primary/10 p-2.5 text-primary">
            <History size={28} />
          </div>
          <div>
            <h1
              className="text-2xl font-semibold tracking-tight text-foreground"
              id="session-heading"
            >
              {defaultHeading}
            </h1>
            <p className="mt-0.5 text-sm text-muted-foreground">
              {defaultDescription} (
              {t(
                'sessionHistory.loadedCountSummary',
                '{{count}} loaded sessions',
                {
                  count: baseSessions.length,
                },
              )}
              {displayRows.length !== baseSessions.length
                ? `, ${t(
                    'sessionHistory.visibleCountSummary',
                    '{{count}} visible in tree',
                    {
                      count: displayRows.length,
                    },
                  )}`
                : ''}
              {hasMoreSessions
                ? `, ${t('sessionHistory.moreAvailable', 'more available')}`
                : ''}
              {filtersActive
                ? `, ${t(
                    'sessionHistory.matchCountSummary',
                    '{{count}} matching filters',
                    { count: matchedSessionCount },
                  )}`
                : ''}
              )
            </p>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={onRefresh}
          disabled={isLoading}
          aria-label={t('sessionHistory.refreshAria', 'Refresh sessions')}
          className="h-9 w-9"
        >
          <RefreshCw className={cn('h-4 w-4', isLoading && 'animate-spin')} />
        </Button>
      </div>

      {!selectedLineageId &&
        !showBookmarkedOnly &&
        bookmarkedSessions.length > 0 && (
          <section
            id="bookmarked-sessions"
            className={cn(
              HISTORY_CONTENT_RAIL_CLASS,
              HISTORY_SECTION_CLASS,
              'mb-4',
            )}
          >
            <div className="flex flex-wrap items-end justify-between gap-3">
              <div className="space-y-1">
                <div className="flex items-center gap-2">
                  <BookmarkCheck className="h-4 w-4 text-warning" />
                  <h2 className="text-sm font-semibold text-foreground">
                    {t(
                      'sessionHistory.bookmarkedSection.heading',
                      'Bookmarked Sessions',
                    )}
                  </h2>
                  <Badge variant="secondary">{bookmarkedSessions.length}</Badge>
                </div>
                <p className="text-xs text-muted-foreground">
                  {t(
                    'sessionHistory.bookmarkedSection.description',
                    'Pinned sessions stay here for quick access while the full history remains below.',
                  )}
                </p>
              </div>
              <div className="flex items-center gap-2">
                {remainingBookmarkedCount > 0 && (
                  <Badge variant="outline" className="h-6 px-2">
                    {t(
                      'sessionHistory.bookmarkedSection.more',
                      '+{{count}} more bookmarked',
                      { count: remainingBookmarkedCount },
                    )}
                  </Badge>
                )}
                {remainingBookmarkedCount > 0 && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={handleBrowseBookmarkedHistory}
                  >
                    {t(
                      'sessionHistory.bookmarkedSection.browseAll',
                      'Browse all bookmarked',
                    )}
                  </Button>
                )}
              </div>
            </div>

            <div className="mt-3 flex flex-col gap-2" role="list">
              {featuredBookmarkedSessions.map((session) => (
                <div key={session.id} role="listitem">
                  <BookmarkedSessionRow
                    session={session}
                    onResume={onResume}
                    onToggleBookmark={onToggleBookmark}
                    t={t}
                  />
                </div>
              ))}
            </div>
          </section>
        )}

      <div
        ref={historyControlsRef}
        className={cn(
          HISTORY_CONTENT_RAIL_CLASS,
          HISTORY_SECTION_CLASS,
          'mb-4 flex flex-col gap-3',
        )}
      >
        <div className="flex flex-col gap-3 md:flex-row md:items-center">
          <div className="relative min-w-0 flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 transform text-muted-foreground" />
            <Input
              type="text"
              placeholder={defaultSearchPlaceholder}
              value={searchQuery}
              onChange={(event) => onSearchQueryChange(event.target.value)}
              className="pl-10 pr-10"
              aria-label={t('sessionHistory.searchAria', 'Search sessions')}
            />
            {searchQuery && (
              <button
                type="button"
                onClick={() => onSearchQueryChange('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 transform rounded-sm text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                aria-label={t('sessionHistory.clearSearchAria', 'Clear search')}
              >
                <X className="h-4 w-4" />
              </button>
            )}
          </div>

          <div className="flex flex-wrap items-center gap-2 md:justify-end">
            <Button
              type="button"
              size="sm"
              variant={showBookmarkedOnly ? 'secondary' : 'outline'}
              className="shrink-0"
              aria-pressed={showBookmarkedOnly}
              onClick={() =>
                setShowBookmarkedOnly((previousState) => !previousState)
              }
            >
              <BookmarkCheck className="mr-2 h-4 w-4" />
              {t('sessionHistory.tabs.bookmarked', 'Bookmarked')} (
              {bookmarkedSessions.length})
            </Button>
          </div>

          <div className="flex items-center gap-2 md:w-64 md:shrink-0">
            <Select
              value={activeSortKey}
              onValueChange={(value) => {
                if (isSessionSortKey(value)) {
                  setActiveSortKey(value);
                }
              }}
            >
              <SelectTrigger
                size="sm"
                className="w-full"
                aria-label={t('sessionHistory.sort.label', 'Sort sessions')}
              >
                <SelectValue>{sortLabelByValue[activeSortKey]}</SelectValue>
              </SelectTrigger>
              <SelectContent>
                {sessionSortValues.map((value) => (
                  <SelectItem key={value} value={value}>
                    {sortLabelByValue[value]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button
              type="button"
              variant="outline"
              size="icon"
              className="h-8 w-8 shrink-0"
              onClick={() =>
                setActiveSortDirection((previousState) =>
                  previousState === 'asc' ? 'desc' : 'asc',
                )
              }
              aria-label={sortDirectionToggleLabel}
            >
              {activeSortDirection === 'asc' ? (
                <ChevronUp className="h-4 w-4" />
              ) : (
                <ChevronDown className="h-4 w-4" />
              )}
            </Button>
          </div>

          <div className="md:w-56 md:shrink-0">
            <Select
              value={activeStatusFilter}
              onValueChange={(value) => {
                if (isSessionStatusFilterValue(value)) {
                  onActiveStatusFilterChange(value);
                }
              }}
            >
              <SelectTrigger
                size="sm"
                className="w-full"
                aria-label={t('sessionHistory.statusFilter.label', 'Status')}
              >
                <SelectValue>
                  {statusFilterLabelByValue[activeStatusFilter]}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                {statusFilterValues.map((value) => (
                  <SelectItem key={value} value={value}>
                    {statusFilterLabelByValue[value]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>
      </div>

      {(selectedLineageId || showBookmarkedOnly) && (
        <div
          className={cn(
            HISTORY_CONTENT_RAIL_CLASS,
            'mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground',
          )}
        >
          {selectedLineageId && (
            <>
              <span>
                {t('sessionHistory.focusedLineage', 'Focused lineage: {{id}}', {
                  id: selectedLineageId.slice(0, 8),
                })}
              </span>
              <Button
                variant="link"
                size="sm"
                className="h-auto p-0"
                onClick={() => setSelectedLineageId(null)}
              >
                {t('sessionHistory.showAllLineages', 'Show all lineages')}
              </Button>
            </>
          )}
          {showBookmarkedOnly && (
            <>
              <span>
                {t(
                  'sessionHistory.bookmarkFilter.focused',
                  'Showing bookmarked sessions',
                )}
              </span>
              <Button
                variant="link"
                size="sm"
                className="h-auto p-0"
                onClick={() => setShowBookmarkedOnly(false)}
              >
                {t('sessionHistory.bookmarkFilter.clear', 'Show all sessions')}
              </Button>
            </>
          )}
        </div>
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
          <div className="flex h-full items-center justify-center">
            <div className="text-center text-muted-foreground">
              {selectedLineageId ? (
                <>
                  <p className="text-sm">
                    {t(
                      'sessionHistory.noSessionsInLineage',
                      'No sessions visible in lineage {{id}}',
                      { id: selectedLineageId.slice(0, 8) },
                    )}
                  </p>
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => setSelectedLineageId(null)}
                    className="mt-2"
                  >
                    {t(
                      'sessionHistory.clearLineageFocus',
                      'Clear lineage focus',
                    )}
                  </Button>
                </>
              ) : searchQuery.trim() ? (
                <>
                  <p className="text-sm">
                    {t(
                      'sessionHistory.noSessionsMatching',
                      'No sessions found matching "{{query}}"',
                      { query: searchQuery },
                    )}
                  </p>
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => onSearchQueryChange('')}
                    className="mt-2"
                  >
                    {t('sessionHistory.clearSearch', 'Clear search')}
                  </Button>
                </>
              ) : showBookmarkedOnly ? (
                <>
                  <p className="text-sm">
                    {t(
                      'sessionHistory.bookmarkFilter.empty',
                      'No bookmarked sessions yet',
                    )}
                  </p>
                  <Button
                    variant="link"
                    size="sm"
                    onClick={() => setShowBookmarkedOnly(false)}
                    className="mt-2"
                  >
                    {t(
                      'sessionHistory.bookmarkFilter.clear',
                      'Show all sessions',
                    )}
                  </Button>
                </>
              ) : (
                <>
                  <p className="text-sm">{defaultEmptyTitle}</p>
                  <p className="mt-2 text-xs">{defaultEmptySubtitle}</p>
                </>
              )}
            </div>
          </div>
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
