import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { AgentResponse } from '@/models/agent-ipc';

const logger = getLogger('AgentSessionListContext');

/**
 * Backend-authoritative pending-approval counts per active session (#2971).
 *
 * The bell keeps counts in frontend state, which drops approval events for
 * sessions that are not in the loaded lists yet. The backend map is the
 * source of truth: sessions absent from the map have no pending approvals
 * (inactive sessions cannot hold them).
 *
 * Returns `null` when the backend query fails, so callers can fall back to
 * frontend state instead of wiping live counts.
 */
export async function fetchPendingApprovalCounts(): Promise<Map<
  string,
  number
> | null> {
  try {
    const response = await safeInvoke<AgentResponse<Record<string, number>>>(
      'agent_get_pending_approval_counts',
    );
    const data = response?.data;
    if (typeof data !== 'object' || data === null) {
      return null;
    }
    return new Map(
      Object.entries(data).filter(
        (entry): entry is [string, number] => typeof entry[1] === 'number',
      ),
    );
  } catch (err) {
    logger.warn('Failed to fetch pending approval counts', err);
    return null;
  }
}

/**
 * Merge backend counts over a frontend-seeded map. Backend values win for
 * every session it knows about; sessions it does not list are treated as 0.
 */
export function mergePendingApprovalCounts(
  frontendCounts: Map<string, number>,
  backendCounts: Map<string, number>,
): Map<string, number> {
  const merged = new Map<string, number>();
  for (const sessionId of frontendCounts.keys()) {
    merged.set(sessionId, backendCounts.get(sessionId) ?? 0);
  }
  backendCounts.forEach((count, sessionId) => {
    merged.set(sessionId, count);
  });
  return merged;
}
