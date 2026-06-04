import type { AgentSession } from '@/models/agent';
import {
  buildChildrenMap,
  filterSessions,
  type SessionStatus,
  type SessionStatusCounts,
} from '@/lib/session-utils';
import {
  compareSessionsBySort,
  compareSessionsByLatestActivityDesc,
  statusPriority,
  type SessionSortKey,
  type SessionSortDirection,
  type SessionHistoryTranslate,
} from './session-history-utils';

export interface SessionHistoryRow {
  session: AgentSession;
  nestingLevel: number;
  lineageHint?: string;
  hasExpandableChildren: boolean;
  isExpanded: boolean;
  descendantStatusCounts?: SessionStatusCounts;
}

interface ComputeSessionTreeParams {
  deferredSessions: AgentSession[];
  selectedLineageId: string | null;
  showBookmarkedOnly: boolean;
  activeStatusFilter: 'all' | SessionStatus;
  deferredSearchQuery: string;
  activeSortKey: SessionSortKey;
  activeSortDirection: SessionSortDirection;
  manuallyExpandedSessionIds: Set<string>;
  collapsedAutoExpandedSessionIds: Set<string>;
  descendantStatusCounts: Map<string, SessionStatusCounts>;
  t: SessionHistoryTranslate;
}

/**
 * Calculates the counts of sessions in each status.
 */
function calculateStatusCounts(sessions: AgentSession[]): {
  all: number;
  busy: number;
  idle: number;
  paused: number;
  error: number;
} {
  const counts = {
    all: sessions.length,
    busy: 0,
    idle: 0,
    paused: 0,
    error: 0,
  };
  sessions.forEach((session) => {
    if (Object.prototype.hasOwnProperty.call(counts, session.status)) {
      counts[session.status as keyof typeof counts]++;
    }
  });
  return counts;
}

/**
 * Filters and sorts sessions based on status, search query, and sorting criteria.
 */
function getMatchedAndSortedSessions({
  sessions,
  activeStatusFilter,
  searchQuery,
  sortKey,
  sortDirection,
  t,
}: {
  sessions: AgentSession[];
  activeStatusFilter: 'all' | SessionStatus;
  searchQuery: string;
  sortKey: SessionSortKey;
  sortDirection: SessionSortDirection;
  t: SessionHistoryTranslate;
}): AgentSession[] {
  let filtered = sessions;
  if (activeStatusFilter !== 'all') {
    filtered = filtered.filter((session) => session.status === activeStatusFilter);
  }

  return [...filterSessions(filtered, searchQuery)].sort((a, b) => {
    const sortDiff = compareSessionsBySort(a, b, sortKey, sortDirection, t);
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
}

/**
 * Resolves all ancestors for matching sessions and marks them as visible/auto-expanded.
 */
function resolveVisibleAndAutoExpandedSessions({
  matchedSessions,
  sessionById,
  filtersActive,
}: {
  matchedSessions: AgentSession[];
  sessionById: Map<string, AgentSession>;
  filtersActive: boolean;
}) {
  const visibleIds = new Set<string>();
  const autoExpandedAncestorIds = new Set<string>();

  matchedSessions.forEach((session) => {
    let current: AgentSession | undefined = session;
    while (current) {
      if (visibleIds.has(current.id)) {
        break;
      }

      visibleIds.add(current.id);
      const parent: AgentSession | undefined = current.parentSessionId
        ? sessionById.get(current.parentSessionId)
        : undefined;
      if (parent && filtersActive) {
        autoExpandedAncestorIds.add(parent.id);
      }
      current = parent;
    }
  });

  return { visibleIds, autoExpandedAncestorIds };
}

/**
 * Creates a comparator function to sort child lists according to parent-child ordering constraints.
 */
function createSessionComparator(
  sortIndexById: Map<string, number>,
  childrenByParent: Map<string, AgentSession[]>,
  visibleIds: Set<string>,
) {
  const orderCache = new Map<string, number>();

  const getOrder = (session: AgentSession): number => {
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
      let minDescendantOrder = Number.MAX_SAFE_INTEGER;
      for (let i = 0; i < descendants.length; i++) {
        const child = descendants[i];
        if (visibleIds.has(child.id)) {
          const childOrder = getOrder(child);
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

  return (a: AgentSession, b: AgentSession) => {
    const orderDiff = getOrder(a) - getOrder(b);
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
}

/**
 * Recursively walks the session tree to build the flat display rows.
 */
function buildDisplayRows({
  roots,
  childrenByParent,
  visibleIds,
  sessionById,
  effectiveExpandedSessionIds,
  descendantStatusCounts,
  t,
}: {
  roots: AgentSession[];
  childrenByParent: Map<string, AgentSession[]>;
  visibleIds: Set<string>;
  sessionById: Map<string, AgentSession>;
  effectiveExpandedSessionIds: Set<string>;
  descendantStatusCounts: Map<string, SessionStatusCounts>;
  t: SessionHistoryTranslate;
}): SessionHistoryRow[] {
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

  return rows;
}

export function computeSessionTree({
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
  t,
}: ComputeSessionTreeParams) {
  // 1. Determine base lineage sessions
  const lineageSessions = selectedLineageId
    ? deferredSessions.filter((session) => session.lineageId === selectedLineageId)
    : deferredSessions;

  // 2. Filter by bookmarks if needed
  const bookmarkedScopeSessions = showBookmarkedOnly
    ? lineageSessions.filter((session) => session.isBookmarked === true)
    : lineageSessions;

  // 3. Count statuses in scope
  const statusCounts = calculateStatusCounts(bookmarkedScopeSessions);

  // 4. Determine derived filtersActive flag
  const filtersActive =
    activeStatusFilter !== 'all' ||
    deferredSearchQuery.trim().length > 0 ||
    showBookmarkedOnly;

  // 5. Filter and sort matched sessions (search/sort keys)
  const matchedSessions = getMatchedAndSortedSessions({
    sessions: bookmarkedScopeSessions,
    activeStatusFilter,
    searchQuery: deferredSearchQuery,
    sortKey: activeSortKey,
    sortDirection: activeSortDirection,
    t,
  });

  // 6. Build index & relationships
  const sessionById = new Map<string, AgentSession>();
  for (let i = 0; i < lineageSessions.length; i++) {
    sessionById.set(lineageSessions[i].id, lineageSessions[i]);
  }
  const childrenByParent = buildChildrenMap(lineageSessions);

  // 7. Resolve visible sessions and automatically expanded ancestors
  const { visibleIds, autoExpandedAncestorIds } = resolveVisibleAndAutoExpandedSessions({
    matchedSessions,
    sessionById,
    filtersActive,
  });

  // 8. Merge manually expanded and auto-expanded ancestors
  const effectiveExpandedSessionIds = new Set(manuallyExpandedSessionIds);
  autoExpandedAncestorIds.forEach((sessionId) => {
    if (!collapsedAutoExpandedSessionIds.has(sessionId)) {
      effectiveExpandedSessionIds.add(sessionId);
    }
  });

  // 9. Sort parent-child lists according to matched search/sorting order
  const sortIndexById = new Map<string, number>();
  for (let i = 0; i < matchedSessions.length; i++) {
    sortIndexById.set(matchedSessions[i].id, i);
  }
  const sortByCurrentOrder = createSessionComparator(
    sortIndexById,
    childrenByParent,
    visibleIds,
  );

  for (const children of childrenByParent.values()) {
    children.sort(sortByCurrentOrder);
  }

  // 10. Extract roots & Walk the tree to build display rows
  const roots = lineageSessions
    .filter((session) => {
      if (!visibleIds.has(session.id)) {
        return false;
      }
      return !session.parentSessionId || !visibleIds.has(session.parentSessionId);
    })
    .sort(sortByCurrentOrder);

  const displayRows = buildDisplayRows({
    roots,
    childrenByParent,
    visibleIds,
    sessionById,
    effectiveExpandedSessionIds,
    descendantStatusCounts,
    t,
  });

  return {
    autoExpandedAncestorIds,
    baseSessions: bookmarkedScopeSessions,
    displayRows,
    matchedSessionCount: matchedSessions.length,
    statusCounts,
  };
}
