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
