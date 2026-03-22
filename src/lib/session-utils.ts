import { AgentSession } from '@/models/agent';

export type SessionStatus = AgentSession['status'];

export interface SessionStatusCounts {
  busy: number;
  idle: number;
  paused: number;
  error: number;
}

function createEmptyStatusCounts(): SessionStatusCounts {
  return {
    busy: 0,
    idle: 0,
    paused: 0,
    error: 0,
  };
}

/**
 * Filters a list of sessions based on a search query.
 * Matches against session name, ID, assistant name, and assistant description.
 *
 * @param sessions - List of sessions to filter
 * @param query - Search query string
 * @returns Filtered list of sessions
 */
export function filterSessions<T extends AgentSession>(
  sessions: T[],
  query: string,
): T[] {
  if (!query || !query.trim()) {
    return sessions;
  }

  const lowerQuery = query.toLowerCase().trim();

  return sessions.filter((session) => {
    const name = session.name?.toLowerCase() || '';
    const id = session.id.toLowerCase();
    const assistantName = session.assistant?.name?.toLowerCase() || '';
    const description = session.assistant?.description?.toLowerCase() || '';

    return (
      name.includes(lowerQuery) ||
      id.includes(lowerQuery) ||
      assistantName.includes(lowerQuery) ||
      description.includes(lowerQuery)
    );
  });
}

export function buildChildrenMap<T extends AgentSession>(
  sessions: T[],
): Map<string, T[]> {
  const childrenMap = new Map<string, T[]>();

  for (const session of sessions) {
    if (!session.parentSessionId) {
      continue;
    }

    const children = childrenMap.get(session.parentSessionId) || [];
    children.push(session);
    childrenMap.set(session.parentSessionId, children);
  }

  return childrenMap;
}

export function buildDescendantCounts<T extends AgentSession>(
  sessions: T[],
): Map<string, number> {
  const counts = new Map<string, number>();
  const childrenMap = buildChildrenMap(sessions);

  const count = (sessionId: string): number => {
    if (counts.has(sessionId)) {
      return counts.get(sessionId) ?? 0;
    }

    const children = childrenMap.get(sessionId) || [];
    const total =
      children.length +
      children.reduce((sum, child) => sum + count(child.id), 0);

    counts.set(sessionId, total);
    return total;
  };

  sessions.forEach((session) => {
    count(session.id);
  });

  return counts;
}

export function buildDescendantStatusCounts<T extends AgentSession>(
  sessions: T[],
): Map<string, SessionStatusCounts> {
  const counts = new Map<string, SessionStatusCounts>();
  const childrenMap = buildChildrenMap(sessions);

  const countStatuses = (sessionId: string): SessionStatusCounts => {
    if (counts.has(sessionId)) {
      return counts.get(sessionId) ?? createEmptyStatusCounts();
    }

    const total = createEmptyStatusCounts();
    const children = childrenMap.get(sessionId) || [];

    for (const child of children) {
      total[child.status] += 1;
      const childCounts = countStatuses(child.id);
      total.busy += childCounts.busy;
      total.idle += childCounts.idle;
      total.paused += childCounts.paused;
      total.error += childCounts.error;
    }

    counts.set(sessionId, total);
    return total;
  };

  sessions.forEach((session) => {
    countStatuses(session.id);
  });

  return counts;
}

export function applyViewedAtToSession<T extends AgentSession>(
  session: T,
  viewedAt: Date,
): T {
  const nextLastViewedAt =
    !session.lastViewedAt || viewedAt.getTime() > session.lastViewedAt.getTime()
      ? viewedAt
      : session.lastViewedAt;
  const shouldClearAttention = Boolean(
    session.lastAttentionAt &&
      viewedAt.getTime() >= session.lastAttentionAt.getTime(),
  );

  return {
    ...session,
    lastViewedAt: nextLastViewedAt,
    lastAttentionAt: shouldClearAttention ? undefined : session.lastAttentionAt,
    lastAttentionReason: shouldClearAttention
      ? undefined
      : session.lastAttentionReason,
  };
}
