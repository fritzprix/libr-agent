import { describe, it, expect } from 'vitest';
import { filterSessions } from '../session-utils';
import { AgentSession } from '@/models/agent';

describe('session-utils', () => {
  describe('filterSessions', () => {
    const mockSessions: AgentSession[] = [
      {
        id: 'session-1',
        name: 'First Session',
        assistant: {
          id: 'assistant-1',
          name: 'Helpful Bot',
          description: 'A bot that helps',
        },
      } as AgentSession,
      {
        id: 'session-2',
        name: 'Second Meeting',
        assistant: {
          id: 'assistant-2',
          name: 'Coding Assistant',
          description: 'Helps with code',
        },
      } as AgentSession,
      {
        id: 'session-3',
        name: null as unknown as string, // Edge case: name is missing/null
        assistant: null as unknown as AgentSession['assistant'], // Edge case: no assistant
      } as AgentSession,
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
