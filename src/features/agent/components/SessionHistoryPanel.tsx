import { useMemo, useState, useDeferredValue } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { RefreshCw, Search, History, X, Bookmark } from 'lucide-react';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { filterSessions } from '@/lib/session-utils';
import type { AgentSession } from '@/models/agent';
import { SessionCard } from './SessionCard';

interface SessionHistoryPanelProps {
  sessions: AgentSession[];
  isLoading: boolean;
  activeTab: string;
  searchQuery: string;
  onActiveTabChange: (value: string) => void;
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
  searchQuery,
  onActiveTabChange,
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
  const [showBookmarkedOnly, setShowBookmarkedOnly] = useState(false);

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

  const filteredAndSortedSessions = useMemo(() => {
    let filtered = baseSessions;

    if (showBookmarkedOnly) {
      filtered = filtered.filter((session) => session.isBookmarked === true);
    }

    if (activeTab !== 'all') {
      filtered = filtered.filter((session) => session.status === activeTab);
    }

    filtered = filterSessions(filtered, deferredSearchQuery);

    return [...filtered].sort((a, b) => {
      const statusDiff =
        (statusPriority[a.status] ?? 999) - (statusPriority[b.status] ?? 999);
      if (statusDiff !== 0) return statusDiff;
      return b.createdAt.getTime() - a.createdAt.getTime();
    });
  }, [baseSessions, deferredSearchQuery, activeTab, showBookmarkedOnly]);

  // Eradicating Action-Effect Chain / Derived State
  // Checking existence of selected lineage directly during render.
  // Only update prevSessions (and check lineage) when a selection is active — avoids
  // an extra re-render on every sessions identity change when nothing is selected.
  const [prevSessions, setPrevSessions] = useState(sessions);

  if (sessions !== prevSessions && selectedLineageId) {
    setPrevSessions(sessions);
    const stillExists = sessions.some(
      (session) => session.lineageId === selectedLineageId,
    );
    if (!stillExists) {
      // Schedule a re-render with the cleared lineage ID
      setSelectedLineageId(null);
    }
  }

  // SP7: Precompute descendant counts for all sessions so SessionCard can warn
  //      users about cascade deletes.  Uses the full (unfiltered) sessions list
  //      so the count stays accurate even when a lineage filter is active.
  //      Optimized to O(N) using an adjacency map.
  //      NOTE: We use the immediate 'sessions' prop instead of 'deferredSessions'
  //      to ensure delete warnings are always based on the latest data.
  const descendantCounts = useMemo(() => {
    const counts = new Map<string, number>();
    const childrenMap = new Map<string, AgentSession[]>();

    // Build adjacency list - O(N)
    for (const session of sessions) {
      if (session.parentSessionId) {
        const children = childrenMap.get(session.parentSessionId) || [];
        children.push(session);
        childrenMap.set(session.parentSessionId, children);
      }
    }

    const count = (sessionId: string): number => {
      if (counts.has(sessionId)) {
        return counts.get(sessionId)!;
      }
      const children = childrenMap.get(sessionId) || [];
      const total =
        children.length +
        children.reduce((sum, child) => sum + count(child.id), 0);
      counts.set(sessionId, total);
      return total;
    };

    sessions.forEach((s) => count(s.id));
    return counts;
  }, [sessions]);

  const displayRows = useMemo(() => {
    type SessionRow = {
      session: AgentSession;
      nestingLevel: number;
      lineageHint?: string;
    };

    const rows: SessionRow[] = [];
    const sortIndexById = new Map(
      filteredAndSortedSessions.map((session, index) => [session.id, index]),
    );
    const sessionById = new Map(
      filteredAndSortedSessions.map((session) => [session.id, session]),
    );
    const childrenByParent = new Map<string, AgentSession[]>();
    const roots: AgentSession[] = [];

    for (const session of filteredAndSortedSessions) {
      const parentId = session.parentSessionId;
      if (parentId && sessionById.has(parentId)) {
        const children = childrenByParent.get(parentId) || [];
        children.push(session);
        childrenByParent.set(parentId, children);
      } else {
        roots.push(session);
      }
    }

    const sortByCurrentOrder = (a: AgentSession, b: AgentSession) => {
      return (sortIndexById.get(a.id) ?? 0) - (sortIndexById.get(b.id) ?? 0);
    };

    roots.sort(sortByCurrentOrder);
    for (const children of childrenByParent.values()) {
      children.sort(sortByCurrentOrder);
    }

    const visited = new Set<string>();

    const walk = (session: AgentSession, nestingLevel: number) => {
      if (visited.has(session.id)) {
        return;
      }
      visited.add(session.id);

      const parentName = session.parentSessionId
        ? sessionById.get(session.parentSessionId)?.name ||
          t('sessionHistory.card.fallbackName', 'Session {{id}}', {
            id: session.parentSessionId.slice(0, 8),
          })
        : undefined;

      rows.push({
        session,
        nestingLevel,
        lineageHint: parentName
          ? t('sessionHistory.lineageHint.child', '↳ Child of {{parentName}}', {
              parentName,
            })
          : t('sessionHistory.lineageHint.topLevel', 'Top-level session'),
      });

      const children = childrenByParent.get(session.id) || [];
      for (const child of children) {
        walk(child, nestingLevel + 1);
      }
    };

    for (const root of roots) {
      walk(root, 0);
    }

    for (const session of filteredAndSortedSessions) {
      if (!visited.has(session.id)) {
        walk(session, 0);
      }
    }

    return rows;
  }, [filteredAndSortedSessions]);

  const statusCounts = useMemo(() => {
    const counts = {
      all: baseSessions.length,
      busy: 0,
      idle: 0,
      paused: 0,
      error: 0,
    };

    baseSessions.forEach((session) => {
      if (Object.prototype.hasOwnProperty.call(counts, session.status)) {
        counts[session.status as keyof typeof counts]++;
      }
    });

    return counts;
  }, [baseSessions]);

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
            <TabsTrigger value="busy" className="flex-1">
              {t('sessionHistory.tabs.busy', 'Busy')} ({statusCounts.busy})
            </TabsTrigger>
            <TabsTrigger value="idle" className="flex-1">
              {t('sessionHistory.tabs.idle', 'Idle')} ({statusCounts.idle})
            </TabsTrigger>
            <TabsTrigger value="paused" className="flex-1">
              {t('sessionHistory.tabs.paused', 'Paused')} ({statusCounts.paused}
              )
            </TabsTrigger>
            <TabsTrigger value="error" className="flex-1">
              {t('sessionHistory.tabs.error', 'Error')} ({statusCounts.error})
            </TabsTrigger>
          </TabsList>
        </Tabs>

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
        <Button
          variant={showBookmarkedOnly ? 'secondary' : 'ghost'}
          size="sm"
          className="self-start"
          onClick={() => setShowBookmarkedOnly((prev) => !prev)}
          aria-pressed={showBookmarkedOnly}
          aria-label={t(
            'sessionHistory.bookmarkFilterAria',
            'Show bookmarked sessions only',
          )}
        >
          <Bookmark
            className={cn(
              'h-3.5 w-3.5 mr-1.5',
              showBookmarkedOnly && 'fill-current text-yellow-500',
            )}
          />
          {t('sessionHistory.bookmarkFilter', 'Bookmarked')}
        </Button>
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
              {displayRows.map(({ session, nestingLevel, lineageHint }) => (
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
                    onLineageSelect={(lineageId) =>
                      setSelectedLineageId((prev) =>
                        prev === lineageId ? null : lineageId,
                      )
                    }
                  />
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
