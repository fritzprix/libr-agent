import { AgentSession } from '@/models/agent';

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

/**
 * efficiently computes the number of descendants for each session in O(N).
 *
 * @param sessions - List of sessions to process
 * @returns Map where key is sessionId and value is total descendant count
 */
export function computeDescendantCounts(
  sessions: AgentSession[],
): Map<string, number> {
  const counts = new Map<string, number>();
  const childrenMap = new Map<string, AgentSession[]>();

  // Build adjacency list - O(N)
  for (const session of sessions) {
    if (session.parentSessionId) {
      const parentId = session.parentSessionId;
      const children = childrenMap.get(parentId) || [];
      children.push(session);
      childrenMap.set(parentId, children);
    }
  }

  const count = (sessionId: string): number => {
    if (counts.has(sessionId)) {
      return counts.get(sessionId)!;
    }

    const children = childrenMap.get(sessionId) || [];
    let total = children.length;
    for (const child of children) {
      total += count(child.id);
    }
    counts.set(sessionId, total);
    return total;
  };

  for (const session of sessions) {
    count(session.id);
  }

  return counts;
}
