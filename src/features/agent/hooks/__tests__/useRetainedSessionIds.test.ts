import { renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import {
  MAX_RETAINED_AGENT_SESSIONS,
  useRetainedSessionIds,
} from '../useRetainedSessionIds';

describe('useRetainedSessionIds', () => {
  it('keeps the active session first and retains recent ids in MRU order', () => {
    const { result, rerender } = renderHook(
      ({ activeId }) => useRetainedSessionIds(activeId),
      { initialProps: { activeId: 's1' } },
    );

    expect(result.current).toEqual(['s1']);

    rerender({ activeId: 's2' });
    expect(result.current).toEqual(['s2', 's1']);

    rerender({ activeId: 's3' });
    expect(result.current).toEqual(['s3', 's2', 's1']);
  });

  it('evicts the oldest session past the retention limit', () => {
    const { result, rerender } = renderHook(
      ({ activeId }) => useRetainedSessionIds(activeId, 2),
      { initialProps: { activeId: 's1' } },
    );

    rerender({ activeId: 's2' });
    rerender({ activeId: 's3' });

    expect(result.current).toEqual(['s3', 's2']);
    expect(result.current).not.toContain('s1');
  });

  it('moves a retained session to the front without duplicating it', () => {
    const { result, rerender } = renderHook(
      ({ activeId }) => useRetainedSessionIds(activeId),
      { initialProps: { activeId: 's1' } },
    );

    rerender({ activeId: 's2' });
    rerender({ activeId: 's3' });
    rerender({ activeId: 's1' });

    expect(result.current).toEqual(['s1', 's3', 's2']);
  });

  it('defaults to MAX_RETAINED_AGENT_SESSIONS', () => {
    expect(MAX_RETAINED_AGENT_SESSIONS).toBe(3);

    const { result, rerender } = renderHook(
      ({ activeId }) => useRetainedSessionIds(activeId),
      { initialProps: { activeId: 'a' } },
    );

    rerender({ activeId: 'b' });
    rerender({ activeId: 'c' });
    rerender({ activeId: 'd' });

    expect(result.current).toHaveLength(MAX_RETAINED_AGENT_SESSIONS);
    expect(result.current).toEqual(['d', 'c', 'b']);
  });
});
