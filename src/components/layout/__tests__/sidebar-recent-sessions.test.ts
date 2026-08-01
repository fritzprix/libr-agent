import { describe, expect, it } from 'vitest';
import type { AgentSession } from '@/models/agent';
import { buildSidebarSessionRows } from '../sidebar-recent-sessions';

function createSession(
  id: string,
  overrides: Partial<AgentSession> = {},
): AgentSession {
  return {
    id,
    name: id,
    status: 'idle',
    model: 'test-model',
    provider: 'test-provider',
    createdAt: new Date('2026-03-20T00:00:00Z'),
    updatedAt: new Date('2026-03-20T00:00:00Z'),
    executionMode: 'normal',
    ...overrides,
  };
}

describe('buildSidebarSessionRows', () => {
  it('returns all roots with no hard cap', () => {
    const sessions = Array.from({ length: 8 }, (_, index) =>
      createSession(`root-${index}`, {
        updatedAt: new Date(`2026-03-2${index}T00:00:00Z`),
      }),
    );

    const rows = buildSidebarSessionRows(sessions, new Set());

    expect(rows).toHaveLength(8);
    expect(rows.every((row) => row.nestingLevel === 0)).toBe(true);
  });

  it('hides children until parent is expanded', () => {
    const sessions = [
      createSession('parent'),
      createSession('child', { parentSessionId: 'parent' }),
    ];

    const collapsed = buildSidebarSessionRows(sessions, new Set());
    expect(collapsed.map((row) => row.session.id)).toEqual(['parent']);
    expect(collapsed[0]?.hasExpandableChildren).toBe(true);
    expect(collapsed[0]?.isExpanded).toBe(false);

    const expanded = buildSidebarSessionRows(sessions, new Set(['parent']));
    expect(expanded.map((row) => row.session.id)).toEqual(['parent', 'child']);
    expect(expanded[0]?.isExpanded).toBe(true);
    expect(expanded[1]?.nestingLevel).toBe(1);
  });

  it('treats sessions with missing parents as roots', () => {
    const sessions = [
      createSession('orphan', { parentSessionId: 'missing-parent' }),
      createSession('root'),
    ];

    const rows = buildSidebarSessionRows(sessions, new Set());
    expect(rows.map((row) => row.session.id)).toEqual(['orphan', 'root']);
    expect(rows.every((row) => row.nestingLevel === 0)).toBe(true);
  });

  it('sorts busy sessions before idle, then by recency', () => {
    const sessions = [
      createSession('idle-new', {
        status: 'idle',
        updatedAt: new Date('2026-03-22T00:00:00Z'),
      }),
      createSession('busy-old', {
        status: 'busy',
        updatedAt: new Date('2026-03-20T00:00:00Z'),
      }),
      createSession('busy-new', {
        status: 'busy',
        updatedAt: new Date('2026-03-21T00:00:00Z'),
      }),
    ];

    const rows = buildSidebarSessionRows(sessions, new Set());
    expect(rows.map((row) => row.session.id)).toEqual([
      'busy-new',
      'busy-old',
      'idle-new',
    ]);
  });

  it('marks expandable when only known direct child count is present', () => {
    const sessions = [createSession('parent')];
    const knownCounts = new Map([['parent', 2]]);

    const rows = buildSidebarSessionRows(sessions, new Set(), knownCounts);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.hasExpandableChildren).toBe(true);
    expect(rows[0]?.isExpanded).toBe(false);
  });
});
