import { describe, expect, it } from 'vitest';
import type { AgentSession } from '@/models/agent';
import { selectOrgSummaries } from '../org-sessions';

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
    createdAt: new Date('2026-04-05T00:00:00Z'),
    yoloMode: false,
    ...overrides,
  };
}

describe('selectOrgSummaries', () => {
  it('keeps only explicit org-created lineages', () => {
    const sessions: AgentSession[] = [
      createSession('solo', 'Solo'),
      createSession('lineage-root', 'Lineage Root', { lineageId: 'lineage-1' }),
      createSession('lineage-child', 'Lineage Child', {
        parentSessionId: 'lineage-root',
        lineageId: 'lineage-1',
      }),
      createSession('org-root', 'Org Root', {
        orgId: 'org-1',
        orgName: 'Research Org',
        orgRootSessionId: 'org-root',
      }),
      createSession('org-child', 'Org Child', {
        parentSessionId: 'org-root',
        lineageId: 'lineage-2',
        depth: 1,
        orgId: 'org-1',
        orgName: 'Research Org',
        orgRootSessionId: 'org-root',
        status: 'busy',
      }),
    ];

    const summaries = selectOrgSummaries(sessions);

    expect(summaries).toHaveLength(1);
    expect(summaries[0].orgId).toBe('org-1');
    expect(summaries[0].orgRootSessionId).toBe('org-root');
    expect(summaries[0].members.map((session) => session.id)).toEqual([
      'org-root',
      'org-child',
    ]);
    expect(summaries[0].busyCount).toBe(1);
  });

  it('rejects partial or invalid org metadata', () => {
    const sessions: AgentSession[] = [
      createSession('partial', 'Partial', {
        orgId: 'org-partial',
        orgName: 'Partial Org',
      }),
      createSession('missing-root', 'Missing Root', {
        orgId: 'org-missing-root',
        orgName: 'Missing Root Org',
        orgRootSessionId: 'root-that-does-not-exist',
      }),
    ];

    expect(selectOrgSummaries(sessions)).toEqual([]);
  });
});
