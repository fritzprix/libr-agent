import {
  forwardRef,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
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
} from 'lucide-react';
import {
  Virtuoso,
  type Components,
  type ItemProps,
  type ListProps,
} from 'react-virtuoso';
import {
  buildChildrenMap,
  buildDescendantCounts,
  buildDescendantStatusCounts,
  filterSessions,
  type SessionStatus,
  type SessionStatusCounts,
} from '@/lib/session-utils';
import { formatRelativeTime } from '@/lib/date-utils';
import { sortSessionsByLatestActivity } from '@/lib/session-metadata';
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

interface BookmarkedSessionTileProps {
  session: AgentSession;
  onResume: (sessionId: string) => void;
  onToggleBookmark?: (sessionId: string) => void;
  t: SessionHistoryTranslate;
}

const HISTORY_CONTENT_RAIL_CLASS = 'mx-auto w-full max-w-4xl';
const HISTORY_SECTION_CLASS =
  'rounded-xl border bg-card/80 p-4 shadow-sm shadow-black/5';

function BookmarkedSessionTile({
  session,
  onResume,
  onToggleBookmark,
  t,
}: BookmarkedSessionTileProps) {
  const shortcutLabel =
    session.name ||
    t('sessionHistory.card.fallbackName', 'Session {{id}}', {
      id: session.id.slice(0, 8),
    });
  const latestActivity =
    formatRelativeTime(session.updatedAt ?? session.createdAt, new Date()) ||
    t('sessionHistory.card.justNow', 'just now');
  const statusLabel = t(
    `sessionHistory.status.${session.status}`,
    session.status,
  );
  const secondaryLabel =
    session.assistant?.name ||
    (session.provider && session.model
      ? `${session.provider}/${session.model}`
      : t(
          'sessionHistory.bookmarkedSection.defaultMeta',
          'Saved for quick access',
        ));

  return (
    <article className="rounded-xl border bg-background/80 p-3 shadow-sm shadow-black/5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-start gap-2">
            <h3 className="min-w-0 flex-1 truncate text-sm font-semibold leading-5 text-foreground">
              {shortcutLabel}
            </h3>
            <Badge
              variant="secondary"
              className="h-5 shrink-0 px-1.5 text-[10px]"
            >
              {statusLabel}
            </Badge>
          </div>
          <p className="mt-1 truncate text-xs text-muted-foreground">
            {secondaryLabel}
          </p>
        </div>
        <Button
          type="button"
          size="icon"
          variant="ghost"
          className="h-7 w-7 shrink-0"
          onClick={() => onToggleBookmark?.(session.id)}
          aria-label={t(
            'sessionHistory.actions.unbookmarkAria',
            'Remove bookmark',
          )}
        >
          <StarOff className="h-4 w-4" aria-hidden="true" />
        </Button>
      </div>

      <div className="mt-3 flex items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
          <Clock3 className="h-3.5 w-3.5 shrink-0" />
          <span className="truncate">
            {t('sessionHistory.bookmarkedSection.lastUsed', 'Used {{time}}', {
              time: latestActivity,
            })}
          </span>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0 px-2"
          onClick={() => onResume(session.id)}
          aria-label={t(
            'sessionHistory.bookmarkedSection.resumeAria',
            'Open bookmarked session {{name}}',
            { name: shortcutLabel },
          )}
        >
          <BookmarkCheck className="h-4 w-4 text-warning" />
          <span>
            {t('sessionHistory.bookmarkedSection.open', 'Open session')}
          </span>
        </Button>
      </div>
    </article>
  );
}

const sessionHistoryVirtuosoComponents: Components<SessionHistoryRow> = {
  List: forwardRef<HTMLDivElement, ListProps>(function SessionHistoryList(
    { children, style, ...props },
    ref,
  ) {
    return (
      <div
        {...props}
        ref={ref}
        role="list"
        aria-labelledby="session-heading"
        style={{ ...style, margin: 0, padding: 0 }}
      >
        {children}
      </div>
    );
  }),
  Item: forwardRef<HTMLDivElement, ItemProps<SessionHistoryRow>>(
    function SessionHistoryItem({ children, style, ...props }, ref) {
      return (
        <div {...props} ref={ref} role="listitem" style={style}>
          <div className="pb-4">{children}</div>
        </div>
      );
    },
  ),
};

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
}: SessionHistoryPanelProps) {
  const { t } = useTranslation('common');
  const [selectedLineageId, setSelectedLineageId] = useState<string | null>(
    null,
  );
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
    activeStatusFilter !== 'all' || deferredSearchQuery.trim().length > 0;

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
  }, [activeStatusFilter, deferredSearchQuery, selectedLineageId]);

  const bookmarkedSessions = useMemo(
    () =>
      sortSessionsByLatestActivity(
        deferredSessions.filter((session) => session.isBookmarked === true),
      ),
    [deferredSessions],
  );
  const featuredBookmarkedSessions = bookmarkedSessions.slice(0, 5);
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
    const nextStatusCounts = {
      all: lineageSessions.length,
      busy: 0,
      idle: 0,
      paused: 0,
      error: 0,
    };

    lineageSessions.forEach((session) => {
      if (
        Object.prototype.hasOwnProperty.call(nextStatusCounts, session.status)
      ) {
        nextStatusCounts[session.status as keyof typeof nextStatusCounts]++;
      }
    });

    let filteredSessions = lineageSessions;

    if (activeStatusFilter !== 'all') {
      filteredSessions = filteredSessions.filter(
        (session) => session.status === activeStatusFilter,
      );
    }

    const matchedSessions = [
      ...filterSessions(filteredSessions, deferredSearchQuery),
    ].sort((a, b) => {
      const statusDiff =
        (statusPriority[a.status] ?? 999) - (statusPriority[b.status] ?? 999);
      if (statusDiff !== 0) return statusDiff;
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
      baseSessions: lineageSessions,
      displayRows: rows,
      matchedSessionCount: matchedSessions.length,
      statusCounts: nextStatusCounts,
    };
  }, [
    activeStatusFilter,
    collapsedAutoExpandedSessionIds,
    descendantStatusCounts,
    deferredSearchQuery,
    deferredSessions,
    filtersActive,
    manuallyExpandedSessionIds,
    selectedLineageId,
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

  return (
    <div className="flex min-h-full flex-col bg-background p-6">
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

      {!selectedLineageId && bookmarkedSessions.length > 0 && (
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
            {remainingBookmarkedCount > 0 && (
              <Badge variant="outline" className="h-6 px-2">
                {t(
                  'sessionHistory.bookmarkedSection.more',
                  '+{{count}} more in history',
                  { count: remainingBookmarkedCount },
                )}
              </Badge>
            )}
          </div>

          <div className="mt-3 grid gap-3 md:grid-cols-2">
            {featuredBookmarkedSessions.map((session) => (
              <BookmarkedSessionTile
                key={session.id}
                session={session}
                onResume={onResume}
                onToggleBookmark={onToggleBookmark}
                t={t}
              />
            ))}
          </div>
        </section>
      )}

      <div
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

          <div className="flex items-center gap-2 md:w-56 md:shrink-0">
            <span className="text-xs font-medium text-muted-foreground">
              {t('sessionHistory.statusFilter.label', 'Status')}
            </span>
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

      {selectedLineageId && (
        <div
          className={cn(
            HISTORY_CONTENT_RAIL_CLASS,
            'mt-3 flex items-center gap-2 text-xs text-muted-foreground',
          )}
        >
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
              ) : (
                <>
                  <p className="text-sm">{defaultEmptyTitle}</p>
                  <p className="mt-2 text-xs">{defaultEmptySubtitle}</p>
                </>
              )}
            </div>
          </div>
        ) : (
          <Virtuoso
            useWindowScroll
            className="w-full pb-4"
            data={displayRows}
            overscan={400}
            computeItemKey={(_index, row) => row.session.id}
            components={{
              ...sessionHistoryVirtuosoComponents,
              Footer: () =>
                hasMoreSessions ? (
                  <div className="flex justify-center py-4">
                    <Button
                      variant="outline"
                      onClick={onLoadMore}
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
                ) : null,
            }}
            itemContent={(
              _index,
              {
                session,
                nestingLevel,
                lineageHint,
                hasExpandableChildren,
                isExpanded,
                descendantStatusCounts: rowDescendantStatusCounts,
              },
            ) => (
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
            )}
          />
        )}
      </div>
    </div>
  );
}
