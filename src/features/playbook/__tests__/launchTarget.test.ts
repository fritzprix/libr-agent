import { describe, expect, it, vi } from 'vitest';

import {
  getPinnedLaunchSessionId,
  resolvePlaybookLaunchTarget,
} from '../launchTarget';

describe('getPinnedLaunchSessionId', () => {
  it('returns null when pin is absent', () => {
    expect(getPinnedLaunchSessionId({})).toBeNull();
    expect(
      getPinnedLaunchSessionId({
        defaultTargetSession: { mode: 'self' },
      }),
    ).toBeNull();
  });

  it('returns the pinned session id as stored', () => {
    expect(
      getPinnedLaunchSessionId({
        defaultTargetSession: {
          mode: 'pin',
          sessionId: 'a1b2c3d4e5',
        },
      }),
    ).toBe('a1b2c3d4e5');
  });
});

describe('resolvePlaybookLaunchTarget', () => {
  it('opens a pinned session when it resolves', async () => {
    const resolveSession = vi.fn(async () => 'a1b2c3d4e5');
    await expect(
      resolvePlaybookLaunchTarget(
        {
          defaultTargetSession: {
            mode: 'pin',
            sessionId: 'a1b2c3d4e5',
          },
        },
        resolveSession,
      ),
    ).resolves.toEqual({ kind: 'pinned', sessionId: 'a1b2c3d4e5' });
    expect(resolveSession).toHaveBeenCalledWith('a1b2c3d4e5');
  });

  it('falls back to new when the pinned session is missing', async () => {
    await expect(
      resolvePlaybookLaunchTarget(
        {
          defaultTargetSession: {
            mode: 'pin',
            sessionId: 'missing000',
          },
        },
        async () => null,
      ),
    ).resolves.toEqual({ kind: 'new', reason: 'session-missing' });
  });
});
