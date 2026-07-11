import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AgentSession } from '@/models/agent';
import {
  fetchKnownDirectChildCounts,
  selectParentsForChildCountLookup,
  useKnownDirectChildCounts,
} from '../use-known-direct-child-counts';

const mockSafeInvoke = vi.fn();

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: (...args: unknown[]) => mockSafeInvoke(...args),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    warn: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const DEBOUNCE_MS = 200;

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
    executionMode: 'normal',
    ...overrides,
  };
}

describe('selectParentsForChildCountLookup', () => {
  it('includes sessions with no loaded children on the current page', () => {
    const sessions = [
      createSession('parent'),
      createSession('child', { parentSessionId: 'other' }),
    ];

    expect(
      selectParentsForChildCountLookup(sessions, false).map(
        (session) => session.id,
      ),
    ).toEqual(['parent', 'child']);
  });

  it('includes parents with loaded children only when more sessions can load', () => {
    const sessions = [
      createSession('parent'),
      createSession('child', { parentSessionId: 'parent' }),
    ];

    expect(
      selectParentsForChildCountLookup(sessions, false).map(
        (session) => session.id,
      ),
    ).toEqual(['child']);
    expect(
      selectParentsForChildCountLookup(sessions, true).map(
        (session) => session.id,
      ),
    ).toEqual(['parent', 'child']);
  });
});

describe('fetchKnownDirectChildCounts', () => {
  beforeEach(() => {
    mockSafeInvoke.mockReset();
  });

  it('fetches counts in chunks without flooding all requests at once', async () => {
    const candidates = Array.from({ length: 12 }, (_, index) =>
      createSession(`parent-${index}`),
    );
    let inFlight = 0;
    let maxInFlight = 0;

    mockSafeInvoke.mockImplementation(async () => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await new Promise((resolve) => setTimeout(resolve, 5));
      inFlight -= 1;
      return ['child-a', 'child-b'];
    });

    const results = await fetchKnownDirectChildCounts(candidates, 10);

    expect(results.size).toBe(12);
    expect(maxInFlight).toBeLessThanOrEqual(10);
    for (const session of candidates) {
      expect(results.get(session.id)).toEqual({ status: 'ok', count: 2 });
    }
  });

  it('returns error results instead of zero counts on invoke failure', async () => {
    mockSafeInvoke.mockRejectedValueOnce(new Error('db unavailable'));

    const results = await fetchKnownDirectChildCounts([
      createSession('parent'),
    ]);

    expect(results.get('parent')).toEqual({ status: 'error' });
  });
});

describe('useKnownDirectChildCounts', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockSafeInvoke.mockReset();
    mockSafeInvoke.mockResolvedValue(['child-1', 'child-2']);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('debounces fetches when sessions change rapidly', async () => {
    const { rerender } = renderHook(
      ({ sessions, hasMoreSessions }) =>
        useKnownDirectChildCounts(sessions, hasMoreSessions),
      {
        initialProps: {
          sessions: [createSession('parent-a')],
          hasMoreSessions: false,
        },
      },
    );

    rerender({
      sessions: [createSession('parent-a'), createSession('parent-b')],
      hasMoreSessions: false,
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    rerender({
      sessions: [
        createSession('parent-a'),
        createSession('parent-b'),
        createSession('parent-c'),
      ],
      hasMoreSessions: false,
    });

    expect(mockSafeInvoke).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DEBOUNCE_MS);
      await Promise.resolve();
    });

    expect(mockSafeInvoke).toHaveBeenCalledTimes(3);
  });

  it('retains previous counts when a refetch fails', async () => {
    const sessions = [createSession('parent')];

    const { result, rerender } = renderHook(
      ({ currentSessions, hasMoreSessions }) =>
        useKnownDirectChildCounts(currentSessions, hasMoreSessions),
      {
        initialProps: {
          currentSessions: sessions,
          hasMoreSessions: false,
        },
      },
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DEBOUNCE_MS);
      await Promise.resolve();
    });

    expect(result.current.get('parent')).toBe(2);

    mockSafeInvoke.mockRejectedValueOnce(new Error('temporary failure'));

    rerender({
      currentSessions: [...sessions, createSession('other-parent')],
      hasMoreSessions: false,
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(DEBOUNCE_MS);
      await Promise.resolve();
    });

    expect(result.current.get('parent')).toBe(2);
  });
});
