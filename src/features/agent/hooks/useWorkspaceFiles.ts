import { useEffect, useRef, useState } from 'react';
import { listWorkspaceFilePaths } from '@/lib/backend/workspace';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useWorkspaceFiles');

/** Depth thresholds based on query specificity. */
function depthForQuery(query: string): number {
  const len = query.length;
  if (len <= 2) return 2;
  if (len <= 4) return 4;
  return 8;
}

const MAX_RESULTS = 10;

/**
 * Fetches and filters workspace file paths for `@file:` autocomplete.
 * Pass `null` when the dropdown is not active — this resets the cache so the
 * next activation always fetches fresh paths (picks up workspace override changes).
 */
export function useWorkspaceFiles(
  sessionId: string | undefined,
  query: string | null,
): string[] {
  const [allPaths, setAllPaths] = useState<string[]>([]);
  const lastDepthRef = useRef<number>(0);

  const [prevSessionId, setPrevSessionId] = useState<string | undefined>(
    sessionId,
  );
  const [prevQueryIsNull, setPrevQueryIsNull] = useState<boolean>(
    query === null,
  );

  // Adjusting State During Render: Reset cache when session changes.
  if (sessionId !== prevSessionId) {
    setPrevSessionId(sessionId);
    lastDepthRef.current = 0; // Safe to mutate non-rendered ref during gated state update
    setAllPaths([]);
  }

  // Adjusting State During Render: Keep track of previous query state.
  const currentQueryIsNull = query === null;
  if (currentQueryIsNull !== prevQueryIsNull) {
    setPrevQueryIsNull(currentQueryIsNull);
    if (currentQueryIsNull) {
      lastDepthRef.current = 0; // Safe to mutate non-rendered ref during gated state update
    }
  }

  useEffect(() => {
    if (!sessionId || query === null) return;
    const depth = depthForQuery(query);
    if (depth <= lastDepthRef.current) return;

    lastDepthRef.current = depth;
    listWorkspaceFilePaths(sessionId, depth)
      .then((paths) => setAllPaths(paths))
      .catch((e: unknown) => logger.warn('Failed to list workspace files', e));
  }, [sessionId, query]);

  if (query === null) return [];

  if (!query) {
    return allPaths.slice(0, MAX_RESULTS);
  }

  const lower = query.toLowerCase();
  return allPaths
    .filter((p) => p.toLowerCase().includes(lower))
    .slice(0, MAX_RESULTS);
}
