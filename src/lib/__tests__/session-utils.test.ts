import { describe, it, expect } from 'vitest';
import { applyViewedAtToSession, filterSessions } from '../session-utils';
import type { AgentSession } from '@/models/agent';
import type { Assistant } from '@/models/chat';

function makeAssistant(overrides: Partial<Assistant> & Pick<Assistant, 'id'>): Assistant {
  const { id, ...restOverrides } = overrides;

  return {
    id,
    name: 'Default Assistant',
    systemPrompt: 'You are helpful.',
    deletionProtected: false,
    createdAt: new Date('2026-03-18T00:00:00.000Z'),
    updatedAt: new Date('2026-03-18T00:00:01.000Z'),
    ...restOverrides,
  };
}

function makeSession(
  overrides: Partial<AgentSession> & Pick<AgentSession, 'id'>,
): AgentSession {
  const { id, ...restOverrides } = overrides;

  return {
    id,
    name: 'Session Name',
    status: 'idle',
    model: 'gpt-5.4',
    provider: 'openai',
    createdAt: new Date('2026-03-18T00:00:00.000Z'),
    updatedAt: new Date('2026-03-18T00:00:01.000Z'),
    assistant: makeAssistant({ id: 'assistant-default' }),
    yoloMode: false,
    ...restOverrides,
  };
}

describe('session-utils', () => {
  describe('applyViewedAtToSession', () => {
    it('clears attention when the viewed timestamp catches up to the attention timestamp', () => {
      const session = makeSession({
        id: 'session-attention',
        lastViewedAt: new Date('2026-03-18T00:00:00.000Z'),
        lastAttentionAt: new Date('2026-03-18T00:01:00.000Z'),
        lastAttentionReason: 'pendingApproval',
      });

      const result = applyViewedAtToSession(
        session,
        new Date('2026-03-18T00:01:00.000Z'),
      );

      expect(result.lastViewedAt?.toISOString()).toBe(
        '2026-03-18T00:01:00.000Z',
      );
      expect(result.lastAttentionAt).toBeUndefined();
      expect(result.lastAttentionReason).toBeUndefined();
    });

    it('preserves newer attention when viewedAt is older than the attention timestamp', () => {
      const attentionAt = new Date('2026-03-18T00:02:00.000Z');
      const session = makeSession({
        id: 'session-still-unread',
        lastViewedAt: new Date('2026-03-18T00:00:00.000Z'),
        lastAttentionAt: attentionAt,
        lastAttentionReason: 'recurringStop',
      });

      const result = applyViewedAtToSession(
        session,
        new Date('2026-03-18T00:01:00.000Z'),
      );

      expect(result.lastViewedAt?.toISOString()).toBe(
        '2026-03-18T00:01:00.000Z',
      );
      expect(result.lastAttentionAt).toBe(attentionAt);
      expect(result.lastAttentionReason).toBe('recurringStop');
    });
  });

  describe('filterSessions', () => {
    const mockSessions: AgentSession[] = [
      makeSession({
        id: 'session-1',
        name: 'First Session',
        assistant: makeAssistant({
          id: 'assistant-1',
          name: 'Helpful Bot',
          description: 'A bot that helps',
          systemPrompt: 'You are helpful.',
          createdAt: new Date('2026-03-18T00:00:00.000Z'),
          updatedAt: new Date('2026-03-18T00:00:01.000Z'),
        }),
      }),
      makeSession({
        id: 'session-2',
        name: 'Second Meeting',
        assistant: makeAssistant({
          id: 'assistant-2',
          name: 'Coding Assistant',
          description: 'Helps with code',
          systemPrompt: 'You are helpful.',
          createdAt: new Date('2026-03-18T00:00:00.000Z'),
          updatedAt: new Date('2026-03-18T00:00:01.000Z'),
        }),
      }),
      makeSession({
        id: 'session-3',
        name: undefined,
        assistant: undefined,
      }),
    ];

    it('returns all sessions if query is empty', () => {
      expect(filterSessions(mockSessions, '')).toEqual(mockSessions);
      expect(filterSessions(mockSessions, '   ')).toEqual(mockSessions);
    });

    it('filters by session name', () => {
      const result = filterSessions(mockSessions, 'first');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-1');
    });

    it('filters by session ID', () => {
      const result = filterSessions(mockSessions, 'session-2');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-2');
    });

    it('filters by assistant name', () => {
      const result = filterSessions(mockSessions, 'coding');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-2');
    });

    it('filters by assistant description', () => {
      const result = filterSessions(mockSessions, 'helps with code');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-2');
    });

    it('is case insensitive', () => {
      const result = filterSessions(mockSessions, 'FIRST');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-1');
    });

    it('handles sessions with missing or null fields', () => {
      const result = filterSessions(mockSessions, 'session-3');
      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('session-3');
    });

    it('returns empty array when no matches found', () => {
      expect(filterSessions(mockSessions, 'nonexistent')).toEqual([]);
    });
  });
});
