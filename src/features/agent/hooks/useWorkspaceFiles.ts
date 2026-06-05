import { useEffect, useRef, useState } from 'react';
import {
  listWorkspaceFilePaths,
  listWorkspaceFilePathsForPath,
} from '@/lib/backend/workspace';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useWorkspaceFiles');

/** Depth thresholds based on query specificity. */
function depthForQuery(query: string): number {
  const len = query.length;
  if (len <= 2) return 2;
  if (len <= 4) return 4;
  return 8;
}

const MAX_RESULTS = 30;

/**
 * Fetches and filters workspace file paths for `@file:` autocomplete.
 * Pass `null` when the dropdown is not active — this resets the cache so the
 * next activation always fetches fresh paths (picks up workspace override changes).
 */
export function useWorkspaceFiles(
  sessionId: string | undefined,
  query: string | null,
  workspacePath?: string | null,
): string[] {
  const [allPaths, setAllPaths] = useState<string[]>([]);
  const lastDepthRef = useRef<number>(0);

  // Reset cache when workspace source changes.
  useEffect(() => {
    lastDepthRef.current = 0;
    setAllPaths([]);
  }, [sessionId, workspacePath]);

  // Reset depth when query becomes null so the next open refetches.
  useEffect(() => {
    if (query === null) {
      lastDepthRef.current = 0;
    }
  }, [query]);

  useEffect(() => {
    if ((!sessionId && !workspacePath) || query === null) return;
    const depth = depthForQuery(query);
    if (depth <= lastDepthRef.current) return;

    lastDepthRef.current = depth;
    const loadPaths = workspacePath
      ? listWorkspaceFilePathsForPath(workspacePath, depth)
      : listWorkspaceFilePaths(sessionId!, depth);

    loadPaths
      .then((paths) => setAllPaths(paths))
      .catch((e: unknown) => logger.warn('Failed to list workspace files', e));
  }, [sessionId, query, workspacePath]);

  if (query === null) return [];

  if (!query) {
    return allPaths.slice(0, MAX_RESULTS);
  }

  const lower = query.toLowerCase();
  return allPaths
    .filter((p) => {
      const pLower = p.toLowerCase();
      if (query.endsWith('/')) {
        return pLower.startsWith(lower);
      }
      return pLower.includes(lower);
    })
    .slice(0, MAX_RESULTS);
}
