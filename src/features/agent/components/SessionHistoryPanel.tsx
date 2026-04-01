import { useDeferredValue, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { RefreshCw, Search, History, X, Bookmark } from 'lucide-react';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  buildChildrenMap,
  buildDescendantCounts,
  buildDescendantStatusCounts,
  filterSessions,
  type SessionStatus,
  type SessionStatusCounts,
} from '@/lib/session-utils';
import type { AgentSession } from '@/models/agent';
import { SessionCard } from './SessionCard';

interface SessionHistoryPanelProps {
  sessions: AgentSession[];
  isLoading: boolean;
  activeTab: string;
  activeStatusFilter: 'all' | SessionStatus;
  searchQuery: string;
  onActiveTabChange: (value: string) => void;
  onActiveStatusFilterChange: (value: 'all' | SessionStatus) => void;
  onSearchQueryChange: (value: string) => void;
  onRefresh: () => void;
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

const statusPriority: Record<string, number> = {
  busy: 1,
  idle: 2,
  paused: 3,
  error: 4,
};

export function SessionHistoryPanel({
  sessions,
  isLoading,
  activeTab,
  activeStatusFilter,
  searchQuery,
  onActiveTabChange,
  onActiveStatusFilterChange,
  onSearchQueryChange,
  onRefresh,
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
  const prevSessionsForLineageRef = useRef(sessions);
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(
    () => new Set(),
  );
  const prevAutoExpandedSignatureRef = useRef('');

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

  const baseSessions = useMemo(() => {
    if (!selectedLineageId) {
      return deferredSessions;
    }

    return deferredSessions.filter(
      (session) => session.lineageId === selectedLineageId,
    );
  }, [deferredSessions, selectedLineageId]);

  const segmentedSessions = useMemo(() => {
    if (activeTab === 'bookmarked') {
      return baseSessions.filter((session) => session.isBookmarked === true);
    }

    return baseSessions;
  }, [activeTab, baseSessions]);

  const matchedSessions = useMemo(() => {
    let filtered = segmentedSessions;

    if (activeStatusFilter !== 'all') {
      filtered = filtered.filter(
        (session) => session.status === activeStatusFilter,
      );
    }

    filtered = filterSessions(filtered, deferredSearchQuery);

    return [...filtered].sort((a, b) => {
      const statusDiff =
        (statusPriority[a.status] ?? 999) - (statusPriority[b.status] ?? 999);
      if (statusDiff !== 0) return statusDiff;
      return b.createdAt.getTime() - a.createdAt.getTime();
    });
  }, [activeStatusFilter, deferredSearchQuery, segmentedSessions]);

  if (sessions !== prevSessionsForLineageRef.current) {
    prevSessionsForLineageRef.current = sessions;
    if (selectedLineageId) {
      const stillExists = sessions.some(
        (session) => session.lineageId === selectedLineageId,
      );
      if (!stillExists) {
        setSelectedLineageId(null);
      }
    }
  }

  const descendantCounts = useMemo(
    () => buildDescendantCounts(sessions),
    [sessions],
  );

  const descendantStatusCounts = useMemo(
    () => buildDescendantStatusCounts(sessions),
    [sessions],
  );

  const filtersActive =
    activeTab === 'bookmarked' ||
    activeStatusFilter !== 'all' ||
    deferredSearchQuery.trim().length > 0;

  const autoExpandedAncestorIds = useMemo(() => {
    if (!filtersActive) {
      return new Set<string>();
    }

    const sessionById = new Map(
      baseSessions.map((session) => [session.id, session]),
    );
    const expandedIds = new Set<string>();

    matchedSessions.forEach((session) => {
      let current = session.parentSessionId
        ? sessionById.get(session.parentSessionId)
        : undefined;
      while (current) {
        expandedIds.add(current.id);
        current = current.parentSessionId
          ? sessionById.get(current.parentSessionId)
          : undefined;
      }
    });

    return expandedIds;
  }, [baseSessions, filtersActive, matchedSessions]);

  const autoExpandedSignature = [...autoExpandedAncestorIds].sort().join(':');
  if (autoExpandedSignature !== prevAutoExpandedSignatureRef.current) {
    prevAutoExpandedSignatureRef.current = autoExpandedSignature;
    if (autoExpandedAncestorIds.size > 0) {
      setExpandedSessionIds((prev) => {
        const hasNewAncestor = [...autoExpandedAncestorIds].some(
          (sessionId) => !prev.has(sessionId),
        );
        if (!hasNewAncestor) {
          return prev;
        }

        const next = new Set(prev);
        autoExpandedAncestorIds.forEach((sessionId) => next.add(sessionId));
        return next;
      });
    }
  }

  const displayRows = useMemo(() => {
    type SessionRow = {
      session: AgentSession;
      nestingLevel: number;
      lineageHint?: string;
      hasExpandableChildren: boolean;
      isExpanded: boolean;
      descendantStatusCounts?: SessionStatusCounts;
    };

    const sessionById = new Map(
      baseSessions.map((session) => [session.id, session]),
    );
    const childrenByParent = buildChildrenMap(baseSessions);
    const visibleIds = new Set<string>();

    matchedSessions.forEach((session) => {
      let current: AgentSession | undefined = session;
      while (current) {
        visibleIds.add(current.id);
        current = current.parentSessionId
          ? sessionById.get(current.parentSessionId)
          : undefined;
      }
    });

    const sortIndexById = new Map(
      matchedSessions.map((session, index) => [session.id, index]),
    );
    const orderCache = new Map<string, number>();
    const orderForSession = (session: AgentSession): number => {
      const cachedOrder = orderCache.get(session.id);
      if (cachedOrder !== undefined) {
        return cachedOrder;
      }

      let computedOrder: number;
      if (sortIndexById.has(session.id)) {
        computedOrder =
          sortIndexById.get(session.id) ?? Number.MAX_SAFE_INTEGER;
      } else {
        const descendants = childrenByParent.get(session.id) || [];
        const descendantOrders = descendants
          .filter((child) => visibleIds.has(child.id))
          .map((child) => orderForSession(child));

        computedOrder =
          descendantOrders.length > 0
            ? Math.min(...descendantOrders)
            : Number.MAX_SAFE_INTEGER;
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

    const roots = baseSessions
      .filter((session) => {
        if (!visibleIds.has(session.id)) {
          return false;
        }

        return (
          !session.parentSessionId || !visibleIds.has(session.parentSessionId)
        );
      })
      .sort(sortByCurrentOrder);

    const rows: SessionRow[] = [];

    const walk = (session: AgentSession, nestingLevel: number) => {
      const visibleChildren = (childrenByParent.get(session.id) || [])
        .filter((child) => visibleIds.has(child.id))
        .sort(sortByCurrentOrder);
      const parentName = session.parentSessionId
        ? sessionById.get(session.parentSessionId)?.name ||
          t('sessionHistory.card.fallbackName', 'Session {{id}}', {
            id: session.parentSessionId.slice(0, 8),
          })
        : undefined;
      const hasExpandableChildren = visibleChildren.length > 0;
      const isExpanded = hasExpandableChildren
        ? expandedSessionIds.has(session.id)
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

    return rows;
  }, [
    baseSessions,
    descendantStatusCounts,
    expandedSessionIds,
    matchedSessions,
    t,
  ]);

  const statusCounts = useMemo(() => {
    const counts = {
      all: baseSessions.length,
      busy: 0,
      idle: 0,
      paused: 0,
      error: 0,
    };

    segmentedSessions.forEach((session) => {
      if (Object.prototype.hasOwnProperty.call(counts, session.status)) {
        counts[session.status as keyof typeof counts]++;
      }
    });

    return counts;
  }, [segmentedSessions]);

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
              <History size={28} />
            </div>
            <div>
              <h1
                className="text-2xl text-foreground font-semibold tracking-tight"
                id="session-heading"
              >
                {defaultHeading}
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                {defaultDescription} ({displayRows.length}/{baseSessions.length}
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

        <Tabs
          defaultValue="all"
          value={activeTab}
          onValueChange={onActiveTabChange}
          className="w-full mb-4"
        >
          <TabsList className="w-full justify-start overflow-x-auto">
            <TabsTrigger value="all" className="flex-1">
              {t('sessionHistory.tabs.all', 'All')} ({statusCounts.all})
            </TabsTrigger>
            <TabsTrigger value="bookmarked" className="flex-1">
              <Bookmark className="mr-1.5 h-3.5 w-3.5" />
              {t('sessionHistory.tabs.bookmarked', 'Bookmarked')} (
              {baseSessions.filter((session) => session.isBookmarked).length})
            </TabsTrigger>
          </TabsList>
        </Tabs>

        <div className="mb-4 flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium text-muted-foreground">
            {t('sessionHistory.statusFilter.label', 'Status')}
          </span>
          <Button
            variant={activeStatusFilter === 'all' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onActiveStatusFilterChange('all')}
            aria-pressed={activeStatusFilter === 'all'}
          >
            {t('sessionHistory.statusFilter.all', 'All statuses')}
          </Button>
          <Button
            variant={activeStatusFilter === 'busy' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onActiveStatusFilterChange('busy')}
            aria-pressed={activeStatusFilter === 'busy'}
          >
            {t('sessionHistory.tabs.busy', 'Busy')} ({statusCounts.busy})
          </Button>
          <Button
            variant={activeStatusFilter === 'idle' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onActiveStatusFilterChange('idle')}
            aria-pressed={activeStatusFilter === 'idle'}
          >
            {t('sessionHistory.tabs.idle', 'Idle')} ({statusCounts.idle})
          </Button>
          <Button
            variant={activeStatusFilter === 'paused' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onActiveStatusFilterChange('paused')}
            aria-pressed={activeStatusFilter === 'paused'}
          >
            {t('sessionHistory.tabs.paused', 'Paused')} ({statusCounts.paused})
          </Button>
          <Button
            variant={activeStatusFilter === 'error' ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => onActiveStatusFilterChange('error')}
            aria-pressed={activeStatusFilter === 'error'}
          >
            {t('sessionHistory.tabs.error', 'Error')} ({statusCounts.error})
          </Button>
        </div>

        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
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
              className="absolute right-3 top-1/2 transform -translate-y-1/2 text-muted-foreground hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-sm"
              aria-label={t('sessionHistory.clearSearchAria', 'Clear search')}
            >
              <X className="h-4 w-4" />
            </button>
          )}
        </div>
        {selectedLineageId && (
          <div className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
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

        {/* Content */}
        <div
          className={cn(
            'flex-1 min-h-0 overflow-y-auto pr-2 pb-4 mt-6 transition-opacity duration-200',
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
            <div className="flex items-center justify-center h-full">
              <div className="text-center text-muted-foreground">
                <RefreshCw className="h-8 w-8 animate-spin mx-auto mb-2" />
                <p className="text-sm">
                  {t('sessionHistory.loading', 'Loading sessions...')}
                </p>
              </div>
            </div>
          ) : displayRows.length === 0 ? (
            <div className="flex items-center justify-center h-full">
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
                    <p className="text-xs mt-2">{defaultEmptySubtitle}</p>
                  </>
                )}
              </div>
            </div>
          ) : (
            <ul
              className="grid grid-cols-1 gap-4 max-w-2xl list-none"
              aria-labelledby="session-heading"
            >
              {displayRows.map(
                ({
                  session,
                  nestingLevel,
                  lineageHint,
                  hasExpandableChildren,
                  isExpanded,
                  descendantStatusCounts: rowDescendantStatusCounts,
                }) => (
                  <li key={session.id}>
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
                      onToggleExpand={(sessionId) =>
                        setExpandedSessionIds((prev) => {
                          const next = new Set(prev);
                          if (next.has(sessionId)) {
                            next.delete(sessionId);
                          } else {
                            next.add(sessionId);
                          }
                          return next;
                        })
                      }
                      onLineageSelect={(lineageId) =>
                        setSelectedLineageId((prev) =>
                          prev === lineageId ? null : lineageId,
                        )
                      }
                    />
                  </li>
                ),
              )}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
