import { useEffect, useMemo, useState } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { AgentSession } from '@/models/agent';

const logger = getLogger('useKnownDirectChildCounts');

const DEBOUNCE_MS = 200;
const FETCH_CHUNK_SIZE = 10;
const ERROR_LOG_THRESHOLD = 3;

type ChildCountFetchResult =
  | { status: 'ok'; count: number }
  | { status: 'error' };

function countLoadedChildren(
  sessions: AgentSession[],
  parentId: string,
): number {
  let count = 0;
  for (const session of sessions) {
    if (session.parentSessionId === parentId) {
      count += 1;
    }
  }
  return count;
}

export function selectParentsForChildCountLookup(
  sessions: AgentSession[],
  hasMoreSessions: boolean,
): AgentSession[] {
  return sessions.filter((session) => {
    const loadedChildCount = countLoadedChildren(sessions, session.id);
    if (loadedChildCount === 0) {
      return true;
    }
    return hasMoreSessions;
  });
}

async function fetchDirectChildCount(
  sessionId: string,
): Promise<ChildCountFetchResult> {
  try {
    const childIds = await safeInvoke<string[]>('agent_get_child_session_ids', {
      sessionId,
    });
    const count = Array.isArray(childIds) ? childIds.length : 0;
    return { status: 'ok', count };
  } catch (error) {
    logger.warn('Failed to fetch direct child count', { sessionId, error });
    return { status: 'error' };
  }
}

export async function fetchKnownDirectChildCounts(
  candidates: AgentSession[],
  chunkSize = FETCH_CHUNK_SIZE,
): Promise<Map<string, ChildCountFetchResult>> {
  const results = new Map<string, ChildCountFetchResult>();

  for (let index = 0; index < candidates.length; index += chunkSize) {
    const chunk = candidates.slice(index, index + chunkSize);
    const chunkResults = await Promise.all(
      chunk.map(async (session) => {
        const result = await fetchDirectChildCount(session.id);
        return [session.id, result] as const;
      }),
    );

    for (const [parentId, result] of chunkResults) {
      results.set(parentId, result);
    }
  }

  return results;
}

function mergeChildCountResults(
  previous: Map<string, number>,
  candidates: AgentSession[],
  results: Map<string, ChildCountFetchResult>,
): Map<string, number> {
  const next = new Map<string, number>();

  for (const session of candidates) {
    const result = results.get(session.id);
    if (result?.status === 'ok') {
      if (result.count > 0) {
        next.set(session.id, result.count);
      }
      continue;
    }

    const previousCount = previous.get(session.id);
    if (previousCount !== undefined) {
      next.set(session.id, previousCount);
    }
  }

  return next;
}

function logFetchFailures(
  candidates: AgentSession[],
  results: Map<string, ChildCountFetchResult>,
): void {
  const failedParentIds = candidates
    .map((session) => session.id)
    .filter((parentId) => results.get(parentId)?.status === 'error');

  if (failedParentIds.length === 0) {
    return;
  }

  if (failedParentIds.length >= ERROR_LOG_THRESHOLD) {
    logger.error('Multiple direct child count fetches failed', {
      failureCount: failedParentIds.length,
      parentIds: failedParentIds,
    });
    return;
  }

  logger.warn('Some direct child count fetches failed', {
    failureCount: failedParentIds.length,
    parentIds: failedParentIds,
  });
}

/**
 * Fetches direct child counts from the DB for parents whose children may not
 * be present in the paginated frontend session list.
 */
export function useKnownDirectChildCounts(
  sessions: AgentSession[],
  hasMoreSessions: boolean,
): Map<string, number> {
  const [knownDirectChildCountByParentId, setKnownDirectChildCountByParentId] =
    useState<Map<string, number>>(() => new Map());

  const candidates = useMemo(
    () => selectParentsForChildCountLookup(sessions, hasMoreSessions),
    [sessions, hasMoreSessions],
  );

  const candidateKey = useMemo(
    () => candidates.map((session) => session.id).join('\0'),
    [candidates],
  );

  useEffect(() => {
    if (candidates.length === 0) {
      setKnownDirectChildCountByParentId(new Map());
      return;
    }

    let cancelled = false;

    const timer = window.setTimeout(() => {
      void (async () => {
        const results = await fetchKnownDirectChildCounts(candidates);

        if (cancelled) {
          return;
        }

        logFetchFailures(candidates, results);

        setKnownDirectChildCountByParentId((previous) =>
          mergeChildCountResults(previous, candidates, results),
        );
      })();
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [candidateKey, candidates]);

  return knownDirectChildCountByParentId;
}
