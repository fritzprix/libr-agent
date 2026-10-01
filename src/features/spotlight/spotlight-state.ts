import { getLogger } from '@/lib/logger';
import { compareVersion, isVersionGreater } from './compare-version';
import { FEATURE_SPOTLIGHTS } from './feature-spotlights';
import {
  DEFAULT_SPOTLIGHT_STATE,
  spotlightStateSchema,
  type Spotlight,
  type SpotlightState,
} from './types';

export const SPOTLIGHT_STATE_KEY = 'libragent:spotlight-state:v1';

const logger = getLogger('spotlight-state');
const RELEASE_MAX_ITEMS = 3;

export function loadSpotlightState(): SpotlightState {
  if (typeof window === 'undefined') {
    return { ...DEFAULT_SPOTLIGHT_STATE };
  }
  try {
    const raw = localStorage.getItem(SPOTLIGHT_STATE_KEY);
    if (!raw) {
      return { ...DEFAULT_SPOTLIGHT_STATE };
    }
    const parsed: unknown = JSON.parse(raw);
    const result = spotlightStateSchema.safeParse(parsed);
    if (!result.success) {
      logger.warn('Invalid spotlight state; resetting', result.error);
      return { ...DEFAULT_SPOTLIGHT_STATE };
    }
    return result.data;
  } catch (error) {
    logger.warn('Failed to load spotlight state', error);
    return { ...DEFAULT_SPOTLIGHT_STATE };
  }
}

export function saveSpotlightState(state: SpotlightState): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    localStorage.setItem(SPOTLIGHT_STATE_KEY, JSON.stringify(state));
  } catch (error) {
    logger.warn('Failed to save spotlight state', error);
  }
}

function isDismissed(state: SpotlightState, id: string): boolean {
  return state.dismissedIds.includes(id);
}

function sortByPriority(a: Spotlight, b: Spotlight): number {
  return a.priority - b.priority;
}

/**
 * Spotlights introduced after lastSeenAppVersion and at or before appVersion.
 * Empty lastSeen → treat as first run of this feature (show top release tips once).
 */
export function pickReleaseSpotlights(
  state: SpotlightState,
  appVersion: string,
  pool: readonly Spotlight[] = FEATURE_SPOTLIGHTS,
): Spotlight[] {
  if (!appVersion.trim()) {
    return [];
  }

  const lastSeen = state.lastSeenAppVersion.trim();
  if (lastSeen && !isVersionGreater(appVersion, lastSeen)) {
    return [];
  }

  const candidates = pool
    .filter((tip) => tip.surfaces.includes('release'))
    .filter((tip) => !isDismissed(state, tip.id))
    .filter((tip) => compareVersion(tip.sinceVersion, appVersion) <= 0)
    .filter((tip) => {
      if (!lastSeen) {
        return true;
      }
      return isVersionGreater(tip.sinceVersion, lastSeen);
    })
    .slice()
    .sort(sortByPriority);

  return candidates.slice(0, RELEASE_MAX_ITEMS);
}

export function pickHubHint(
  state: SpotlightState,
  pool: readonly Spotlight[] = FEATURE_SPOTLIGHTS,
): Spotlight | null {
  const candidates = pool
    .filter((tip) => tip.surfaces.includes('hub'))
    .filter((tip) => !isDismissed(state, tip.id))
    .slice()
    .sort(sortByPriority);

  if (candidates.length === 0) {
    return null;
  }

  const index = state.hubHintIndex % candidates.length;
  return candidates[index] ?? null;
}

/**
 * Next wait tip for the composer busy strip (rotates via waitTipIndex).
 */
export function pickWaitTip(
  state: SpotlightState,
  pool: readonly Spotlight[] = FEATURE_SPOTLIGHTS,
): Spotlight | null {
  return selectWaitCandidate(state, pool);
}

function selectWaitCandidate(
  state: SpotlightState,
  pool: readonly Spotlight[],
): Spotlight | null {
  const candidates = pool
    .filter((tip) => tip.surfaces.includes('wait'))
    .filter((tip) => !isDismissed(state, tip.id))
    .slice()
    .sort(sortByPriority);

  if (candidates.length === 0) {
    return null;
  }

  const index = state.waitTipIndex % candidates.length;
  return candidates[index] ?? null;
}

/**
 * Persist then return next state. Callers should feed the return value into
 * React setState — localStorage is written here only (setState does not persist).
 */
export function markReleaseSeen(
  state: SpotlightState,
  appVersion: string,
): SpotlightState {
  const next: SpotlightState = {
    ...state,
    lastSeenAppVersion: appVersion,
  };
  saveSpotlightState(next);
  return next;
}

/**
 * Persist then return next state. Callers should feed the return value into
 * React setState — localStorage is written here only (setState does not persist).
 */
export function dismissSpotlight(
  state: SpotlightState,
  tipId: string,
): SpotlightState {
  if (state.dismissedIds.includes(tipId)) {
    return state;
  }
  const next: SpotlightState = {
    ...state,
    dismissedIds: [...state.dismissedIds, tipId],
  };
  saveSpotlightState(next);
  return next;
}

/**
 * Persist then return next state. Callers should feed the return value into
 * React setState — localStorage is written here only (setState does not persist).
 */
export function rotateHubHint(state: SpotlightState): SpotlightState {
  const next: SpotlightState = {
    ...state,
    hubHintIndex: state.hubHintIndex + 1,
  };
  saveSpotlightState(next);
  return next;
}

/**
 * Advance wait tip rotation after a tip is revealed on the composer strip.
 * Persist then return next state (localStorage write only happens here).
 */
export function recordWaitTipShown(state: SpotlightState): SpotlightState {
  const next: SpotlightState = {
    ...state,
    waitTipIndex: state.waitTipIndex + 1,
  };
  saveSpotlightState(next);
  return next;
}

export function hrefToPath(href: Spotlight['href']): string {
  if (href.type === 'settings') {
    return `/settings?tab=${href.tab}`;
  }
  return href.path;
}
