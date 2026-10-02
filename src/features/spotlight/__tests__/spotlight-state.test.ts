import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest';
import { compareVersion, isVersionGreater } from '../compare-version';
import type { Spotlight, SpotlightState } from '../types';
import {
  dismissSpotlight,
  loadSpotlightState,
  markReleaseSeen,
  pickHubHint,
  pickReleaseSpotlights,
  pickWaitTip,
  recordWaitTipShown,
  rotateHubHint,
  SPOTLIGHT_STATE_KEY,
} from '../spotlight-state';

const emptyState = (): SpotlightState => ({
  lastSeenAppVersion: '',
  dismissedIds: [],
  hubHintIndex: 0,
  waitTipIndex: 0,
});

const pool: Spotlight[] = [
  {
    id: 'a',
    sinceVersion: '0.9.10',
    titleKey: 't.a',
    bodyKey: 'b.a',
    ctaLabelKey: 'c.a',
    href: { type: 'route', path: '/a' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 1,
  },
  {
    id: 'b',
    sinceVersion: '0.9.20',
    titleKey: 't.b',
    bodyKey: 'b.b',
    ctaLabelKey: 'c.b',
    href: { type: 'settings', tab: 'system' },
    surfaces: ['release', 'hub', 'wait'],
    priority: 2,
  },
  {
    id: 'c',
    sinceVersion: '0.9.5',
    titleKey: 't.c',
    bodyKey: 'b.c',
    ctaLabelKey: 'c.c',
    href: { type: 'route', path: '/c' },
    surfaces: ['hub'],
    priority: 3,
  },
];

describe('compareVersion', () => {
  it('orders dotted versions', () => {
    expect(compareVersion('0.9.10', '0.9.9')).toBeGreaterThan(0);
    expect(compareVersion('0.9.21', '0.9.21')).toBe(0);
    expect(isVersionGreater('0.9.22', '0.9.21')).toBe(true);
  });
});

function createMemoryStorage() {
  const store = new Map<string, string>();
  return {
    getItem: vi.fn((key: string) => store.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store.set(key, String(value));
    }),
    removeItem: vi.fn((key: string) => {
      store.delete(key);
    }),
    clear: vi.fn(() => {
      store.clear();
    }),
  };
}

describe('spotlight-state', () => {
  beforeEach(() => {
    vi.stubGlobal('localStorage', createMemoryStorage());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('loads default state when empty', () => {
    expect(loadSpotlightState()).toEqual(emptyState());
  });

  it('fills wait tip fields when loading legacy state blobs', () => {
    localStorage.setItem(
      SPOTLIGHT_STATE_KEY,
      JSON.stringify({
        lastSeenAppVersion: '0.9.20',
        dismissedIds: [],
        hubHintIndex: 2,
      }),
    );
    expect(loadSpotlightState()).toEqual({
      lastSeenAppVersion: '0.9.20',
      dismissedIds: [],
      hubHintIndex: 2,
      waitTipIndex: 0,
    });
  });

  it('resets invalid stored JSON', () => {
    localStorage.setItem(SPOTLIGHT_STATE_KEY, '{"dismissedIds":"nope"}');
    expect(loadSpotlightState().dismissedIds).toEqual([]);
  });

  it('picks top release tips on first run', () => {
    const items = pickReleaseSpotlights(emptyState(), '0.9.21', pool);
    expect(items.map((t) => t.id)).toEqual(['a', 'b']);
  });

  it('only shows tips newer than lastSeen on upgrade', () => {
    const items = pickReleaseSpotlights(
      { ...emptyState(), lastSeenAppVersion: '0.9.15' },
      '0.9.21',
      pool,
    );
    expect(items.map((t) => t.id)).toEqual(['b']);
  });

  it('returns no release tips when already seen this version', () => {
    expect(
      pickReleaseSpotlights(
        { ...emptyState(), lastSeenAppVersion: '0.9.21' },
        '0.9.21',
        pool,
      ),
    ).toEqual([]);
  });

  it('rotates hub hints and skips dismissed', () => {
    let state = { ...emptyState(), lastSeenAppVersion: '0.9.21' };
    expect(pickHubHint(state, pool)?.id).toBe('a');
    state = rotateHubHint(state);
    expect(pickHubHint(state, pool)?.id).toBe('b');
    state = dismissSpotlight(state, 'a');
    state = dismissSpotlight(state, 'b');
    expect(pickHubHint(state, pool)?.id).toBe('c');
  });

  it('persists markReleaseSeen', () => {
    const next = markReleaseSeen(emptyState(), '0.9.21');
    expect(next.lastSeenAppVersion).toBe('0.9.21');
    expect(loadSpotlightState().lastSeenAppVersion).toBe('0.9.21');
  });

  it('rotates wait tips across busy waits', () => {
    let state = emptyState();
    expect(pickWaitTip(state, pool)?.id).toBe('a');
    state = recordWaitTipShown(state);
    expect(state.waitTipIndex).toBe(1);
    expect(pickWaitTip(state, pool)?.id).toBe('b');
    state = recordWaitTipShown(state);
    expect(pickWaitTip(state, pool)?.id).toBe('a');
  });
});
