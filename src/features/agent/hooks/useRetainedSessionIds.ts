import { useEffect, useState } from 'react';

/** How many recently visited sessions keep their AgentSessionProvider mounted. */
export const MAX_RETAINED_AGENT_SESSIONS = 3;

/**
 * MRU list of session ids to keep mounted for instant switch-back.
 * Production callers omit `maxRetained` (uses MAX_RETAINED_AGENT_SESSIONS).
 * Tests may pass a smaller window to assert eviction without changing the product default.
 */
export function useRetainedSessionIds(
  activeSessionId: string,
  maxRetained: number = MAX_RETAINED_AGENT_SESSIONS,
): string[] {
  const [retainedIds, setRetainedIds] = useState<string[]>([activeSessionId]);

  useEffect(() => {
    setRetainedIds((previous) => {
      const withoutActive = previous.filter((id) => id !== activeSessionId);
      const next = [activeSessionId, ...withoutActive].slice(0, maxRetained);
      if (
        next.length === previous.length &&
        next.every((id, index) => id === previous[index])
      ) {
        return previous;
      }
      return next;
    });
  }, [activeSessionId, maxRetained]);

  return retainedIds;
}
