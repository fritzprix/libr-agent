import { describe, expect, it } from 'vitest';
import type { AgentSession } from '@/models/agent';
import { computeSessionTree } from '../session-tree';
import type { SessionHistoryTranslate } from '../session-history-utils';

const t = ((key: string, defaultString?: string) =>
  defaultString ?? key) as SessionHistoryTranslate;

function createSession(
  id: string,
  name: string,
  overrides: Partial<AgentSession> = {},
): AgentSession {
  return {
    id,
    name,
    status: 'idle',
    model: 'test-model',
    provider: 'test-provider',
    createdAt: new Date('2026-03-20T00:00:00Z'),
    executionMode: 'normal',
    ...overrides,
  };
}

describe('computeSessionTree', () => {
  it('shows expand for parents with filtered-out children', () => {
    const parentId = 'parent';
    const sessions: AgentSession[] = [
      createSession(parentId, 'Parent', { status: 'busy', lineageId: parentId }),
      createSession('child-1', 'Child 1', {
        parentSessionId: parentId,
        lineageId: parentId,
        status: 'idle',
      }),
      createSession('child-2', 'Child 2', {
        parentSessionId: parentId,
        lineageId: parentId,
        status: 'idle',
      }),
    ];

    const { displayRows } = computeSessionTree({
      deferredSessions: sessions,
      selectedLineageId: null,
      showBookmarkedOnly: false,
      activeStatusFilter: 'busy',
      deferredSearchQuery: '',
      activeSortKey: 'updatedAt',
      activeSortDirection: 'desc',
      manuallyExpandedSessionIds: new Set(),
      collapsedAutoExpandedSessionIds: new Set(),
      descendantStatusCounts: new Map(),
      t,
    });

    const parentRow = displayRows.find((row) => row.session.id === parentId);
    expect(parentRow?.hasExpandableChildren).toBe(true);
    expect(parentRow?.totalChildrenCount).toBe(2);
    expect(parentRow?.hiddenChildrenCount).toBe(2);
    expect(parentRow?.unloadedChildrenCount).toBe(0);
  });

  it('uses known direct child counts when children are not loaded', () => {
    const parentId = 'parent';
    const sessions: AgentSession[] = [
      createSession(parentId, 'Parent', { status: 'busy', lineageId: parentId }),
    ];

    const knownDirectChildCountByParentId = new Map([[parentId, 3]]);

    const { displayRows } = computeSessionTree({
      deferredSessions: sessions,
      selectedLineageId: null,
      showBookmarkedOnly: false,
      activeStatusFilter: 'all',
      deferredSearchQuery: '',
      activeSortKey: 'updatedAt',
      activeSortDirection: 'desc',
      manuallyExpandedSessionIds: new Set(),
      collapsedAutoExpandedSessionIds: new Set(),
      descendantStatusCounts: new Map(),
      knownDirectChildCountByParentId,
      t,
    });

    const parentRow = displayRows.find((row) => row.session.id === parentId);
    expect(parentRow?.hasExpandableChildren).toBe(true);
    expect(parentRow?.totalChildrenCount).toBe(3);
    expect(parentRow?.unloadedChildrenCount).toBe(3);
    expect(parentRow?.hiddenChildrenCount).toBe(0);
  });

  it('hides children hidden by search while keeping expand visible', () => {
    const parentId = 'parent';
    const sessions: AgentSession[] = [
      createSession(parentId, 'Parent Org', { lineageId: parentId }),
      createSession('child', 'Different Name', {
        parentSessionId: parentId,
        lineageId: parentId,
      }),
    ];

    const { displayRows } = computeSessionTree({
      deferredSessions: sessions,
      selectedLineageId: null,
      showBookmarkedOnly: false,
      activeStatusFilter: 'all',
      deferredSearchQuery: 'Parent Org',
      activeSortKey: 'updatedAt',
      activeSortDirection: 'desc',
      manuallyExpandedSessionIds: new Set(),
      collapsedAutoExpandedSessionIds: new Set(),
      descendantStatusCounts: new Map(),
      t,
    });

    const parentRow = displayRows.find((row) => row.session.id === parentId);
    expect(parentRow?.hasExpandableChildren).toBe(true);
    expect(parentRow?.hiddenChildrenCount).toBe(1);
  });
});
