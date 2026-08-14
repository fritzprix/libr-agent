import type { AgentOpenSessionResponse } from '@/models/agent-ipc';

/** In-memory LRU of last-open payloads. Keep-alive holds 3 providers; this
 * covers a few extra hops so a remount can paint before openAgentSession. */
export const MAX_CACHED_SESSIONS = 8;

const cache = new Map<string, AgentOpenSessionResponse>();

function touch(sessionId: string, response: AgentOpenSessionResponse): void {
  cache.delete(sessionId);
  cache.set(sessionId, response);
  while (cache.size > MAX_CACHED_SESSIONS) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey === undefined) {
      break;
    }
    cache.delete(oldestKey);
  }
}

/** Last successful open payload for a session (LRU, in-memory only). */
export function getOpenSessionView(
  sessionId: string,
): AgentOpenSessionResponse | undefined {
  const cached = cache.get(sessionId);
  if (!cached) {
    return undefined;
  }
  // Refresh LRU order on read so recently revisited sessions stay warm.
  touch(sessionId, cached);
  return cached;
}

export function putOpenSessionView(
  sessionId: string,
  response: AgentOpenSessionResponse,
): void {
  touch(sessionId, response);
}

export function invalidateOpenSessionView(sessionId: string): void {
  cache.delete(sessionId);
}

export function clearOpenSessionViewCache(): void {
  cache.clear();
}

export function isWarmOpenSessionView(
  response: AgentOpenSessionResponse,
): boolean {
  return (
    response.runtimeState.proxy.ready &&
    response.runtimeState.phase !== 'failed' &&
    response.runtimeState.phase !== 'hydrating' &&
    response.runtimeState.phase !== 'not_started'
  );
}
