import { describe, it, expect } from 'vitest';
import { filterSessions } from '../session-utils';
import { AgentSession } from '@/models/agent';

describe('filterSessions', () => {
  const mockSessions: AgentSession[] = [
    {
      id: 'session-1',
      name: 'Project Alpha',
      status: 'idle',
      model: 'gpt-4',
      provider: 'openai',
      createdAt: new Date(),
      assistant: {
        id: 'asst-1',
        name: 'Coder',
        description: 'Helps with coding',
        systemPrompt: '',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
    },
    {
      id: 'session-2',
      name: 'Project Beta',
      status: 'busy',
      model: 'gpt-4',
      provider: 'openai',
      createdAt: new Date(),
      assistant: {
        id: 'asst-2',
        name: 'Writer',
        description: 'Helps with writing',
        systemPrompt: '',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
    },
    {
      id: 'uuid-123-abc',
      name: undefined, // No name
      status: 'idle',
      model: 'gpt-4',
      provider: 'openai',
      createdAt: new Date(),
      assistant: {
        id: 'asst-3',
        name: 'Planner',
        description: 'Project management',
        systemPrompt: '',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
    },
  ];

  it('returns all sessions if query is empty', () => {
    expect(filterSessions(mockSessions, '')).toHaveLength(3);
    expect(filterSessions(mockSessions, '   ')).toHaveLength(3);
  });

  it('filters by session name', () => {
    expect(filterSessions(mockSessions, 'Alpha')).toHaveLength(1);
    expect(filterSessions(mockSessions, 'Alpha')[0].id).toBe('session-1');
  });

  it('filters by session id', () => {
    expect(filterSessions(mockSessions, 'uuid')).toHaveLength(1);
    expect(filterSessions(mockSessions, 'uuid')[0].id).toBe('uuid-123-abc');
  });

  it('filters by assistant name', () => {
    expect(filterSessions(mockSessions, 'Coder')).toHaveLength(1);
    expect(filterSessions(mockSessions, 'Coder')[0].id).toBe('session-1');
  });

  it('filters by assistant description', () => {
    expect(filterSessions(mockSessions, 'writing')).toHaveLength(1);
    expect(filterSessions(mockSessions, 'writing')[0].id).toBe('session-2');
  });

  it('is case insensitive', () => {
    expect(filterSessions(mockSessions, 'alpha')).toHaveLength(1);
    expect(filterSessions(mockSessions, 'CODER')).toHaveLength(1);
  });

  it('matches partial strings', () => {
    expect(filterSessions(mockSessions, 'oj')).toHaveLength(3); // "Project" matches
  });

  it('returns empty array if no match found', () => {
    expect(filterSessions(mockSessions, 'xyz123')).toHaveLength(0);
  });
});
