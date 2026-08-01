import type { AgentSession } from '@/models/agent';
import { buildChildrenMap } from '@/lib/session-utils';

const STATUS_PRIORITY: Record<string, number> = {
  busy: 1,
  idle: 2,
  paused: 3,
  error: 4,
};

export interface SidebarSessionRow {
  session: AgentSession;
  nestingLevel: number;
  hasExpandableChildren: boolean;
  isExpanded: boolean;
}

function compareSessionsBySidebarPriority(
  a: AgentSession,
  b: AgentSession,
): number {
  const statusDiff =
    (STATUS_PRIORITY[a.status] ?? 9) - (STATUS_PRIORITY[b.status] ?? 9);
  if (statusDiff !== 0) {
    return statusDiff;
  }
  return (
    (b.updatedAt ?? b.createdAt).getTime() -
    (a.updatedAt ?? a.createdAt).getTime()
  );
}

/**
 * Builds visible Recent Sessions rows for the app sidebar.
 * Roots are sorted by status priority then recency. Children appear only when
 * their parent id is in `expandedSessionIds`.
 */
export function buildSidebarSessionRows(
  sessions: AgentSession[],
  expandedSessionIds: Set<string>,
  knownDirectChildCountByParentId: Map<string, number> = new Map(),
): SidebarSessionRow[] {
  const sortedSessions = [...sessions].sort(compareSessionsBySidebarPriority);
  const sessionById = new Map(
    sortedSessions.map((session) => [session.id, session]),
  );
  const childrenByParent = buildChildrenMap(sortedSessions);

  for (const children of childrenByParent.values()) {
    children.sort(compareSessionsBySidebarPriority);
  }

  const roots = sortedSessions.filter(
    (session) =>
      !session.parentSessionId || !sessionById.has(session.parentSessionId),
  );

  const rows: SidebarSessionRow[] = [];

  const walk = (session: AgentSession, nestingLevel: number) => {
    const loadedChildren = childrenByParent.get(session.id) ?? [];
    const knownDirectChildCount =
      knownDirectChildCountByParentId.get(session.id) ?? 0;
    const hasExpandableChildren =
      loadedChildren.length > 0 || knownDirectChildCount > 0;
    const isExpanded =
      hasExpandableChildren && expandedSessionIds.has(session.id);

    rows.push({
      session,
      nestingLevel,
      hasExpandableChildren,
      isExpanded,
    });

    if (!isExpanded) {
      return;
    }

    loadedChildren.forEach((child) => {
      walk(child, nestingLevel + 1);
    });
  };

  roots.forEach((root) => {
    walk(root, 0);
  });

  return rows;
}
