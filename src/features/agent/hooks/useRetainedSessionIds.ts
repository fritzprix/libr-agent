import { useCallback, useEffect, useRef, useState } from 'react';

/** How many recently visited sessions keep their AgentSessionProvider mounted. */
export const MAX_RETAINED_AGENT_SESSIONS = 3;

export interface RetainedSessionIds {
  retainedIds: string[];
  /** Drop a non-active session (e.g. deleted) so its provider unmounts. */
  evictSessionId: (sessionId: string) => void;
}

/**
 * MRU list of session ids to keep mounted for instant switch-back.
 * Production callers omit `maxRetained` (uses MAX_RETAINED_AGENT_SESSIONS).
 * Tests may pass a smaller window to assert eviction without changing the product default.
 */
export function useRetainedSessionIds(
  activeSessionId: string,
  maxRetained: number = MAX_RETAINED_AGENT_SESSIONS,
): RetainedSessionIds {
  const [retainedIds, setRetainedIds] = useState<string[]>([activeSessionId]);
  const activeSessionIdRef = useRef(activeSessionId);
  activeSessionIdRef.current = activeSessionId;

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

  const evictSessionId = useCallback((sessionId: string) => {
    if (sessionId === activeSessionIdRef.current) {
      return;
    }
    setRetainedIds((previous) =>
      previous.includes(sessionId)
        ? previous.filter((id) => id !== sessionId)
        : previous,
    );
  }, []);

  return { retainedIds, evictSessionId };
}
