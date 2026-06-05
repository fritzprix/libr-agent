import type { TFunction } from 'i18next';
import type { AgentSession } from '@/models/agent';
import type { SessionStatus } from '@/lib/session-utils';
import { getLatestSessionActivityTimestamp } from '@/lib/session-metadata';

export type SessionSortKey = 'updatedAt' | 'createdAt' | 'name';
export type SessionSortDirection = 'asc' | 'desc';
export type SessionHistoryTranslate = TFunction;

export const HISTORY_CONTENT_RAIL_CLASS = 'mx-auto w-full max-w-4xl';
export const HISTORY_SECTION_CLASS =
  'rounded-xl border bg-card/80 p-4 shadow-sm shadow-black/5';
export const BOOKMARK_PREVIEW_LIMIT = 6;
export const TREE_INDENT_PX = 18;
export const MAX_TREE_INDENT_LEVEL = 8;
export const sessionSortValues: SessionSortKey[] = [
  'updatedAt',
  'createdAt',
  'name',
];

export const statusPriority: Record<string, number> = {
  busy: 1,
  idle: 2,
  paused: 3,
  error: 4,
};

export const statusFilterValues: Array<'all' | SessionStatus> = [
  'all',
  'busy',
  'idle',
  'paused',
  'error',
];

export function getSessionDisplayName(
  session: AgentSession,
  t: SessionHistoryTranslate,
): string {
  return (
    session.name ||
    t('sessionHistory.card.fallbackName', 'Session {{id}}', {
      id: session.id.slice(0, 8),
    })
  );
}

export function isSessionSortKey(value: string): value is SessionSortKey {
  return sessionSortValues.includes(value as SessionSortKey);
}

export function getSessionSortTimestamp(
  session: AgentSession,
  sortKey: Extract<SessionSortKey, 'updatedAt' | 'createdAt'>,
): number {
  if (sortKey === 'updatedAt') {
    return getLatestSessionActivityTimestamp(session);
  }

  return session.createdAt.getTime();
}

export function compareSessionsBySort(
  left: AgentSession,
  right: AgentSession,
  sortKey: SessionSortKey,
  sortDirection: SessionSortDirection,
  t: SessionHistoryTranslate,
): number {
  let comparison = 0;

  if (sortKey === 'name') {
    comparison = getSessionDisplayName(left, t).localeCompare(
      getSessionDisplayName(right, t),
      undefined,
      {
        numeric: true,
        sensitivity: 'base',
      },
    );
  } else {
    comparison =
      getSessionSortTimestamp(left, sortKey) -
      getSessionSortTimestamp(right, sortKey);
  }

  if (comparison === 0) {
    return 0;
  }

  return sortDirection === 'asc' ? comparison : -comparison;
}

export function compareSessionsByLatestActivityDesc(
  left: AgentSession,
  right: AgentSession,
): number {
  return (
    getSessionSortTimestamp(right, 'updatedAt') -
    getSessionSortTimestamp(left, 'updatedAt')
  );
}

export function isSessionStatusFilterValue(
  value: string,
): value is 'all' | SessionStatus {
  return statusFilterValues.includes(value as 'all' | SessionStatus);
}
