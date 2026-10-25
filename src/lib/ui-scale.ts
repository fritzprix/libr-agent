import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getLogger } from '@/lib/logger';

const logger = getLogger('ui-scale');

/** Fixed zoom steps offered in Settings and walked by Ctrl/Cmd +/-. */
export const UI_SCALE_STEPS = [0.9, 1, 1.1, 1.25, 1.5] as const;
export const DEFAULT_UI_SCALE = 1.1;
export const RESET_UI_SCALE = 1;

const UI_SCALE_CACHE_KEY = 'libragent.uiScale';

/** Snap any stored value to a supported step; unknown values fall back to the default. */
export function normalizeUiScale(value: unknown): number {
  return typeof value === 'number' &&
    (UI_SCALE_STEPS as readonly number[]).includes(value)
    ? value
    : DEFAULT_UI_SCALE;
}

/** Next step up or down from `current`, clamped to the ends. */
export function stepUiScale(current: number, direction: 1 | -1): number {
  const index = UI_SCALE_STEPS.indexOf(
    normalizeUiScale(current) as (typeof UI_SCALE_STEPS)[number],
  );
  const next = Math.min(
    UI_SCALE_STEPS.length - 1,
    Math.max(0, index + direction),
  );
  return UI_SCALE_STEPS[next];
}

/**
 * Map a keydown to a UI scale action, or null when it is not a zoom shortcut.
 * `=` is accepted so Ctrl/Cmd + works without Shift on US layouts.
 */
export function resolveUiScaleShortcut(
  event: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'
  >,
): 'up' | 'down' | 'reset' | null {
  if (!(event.metaKey || event.ctrlKey) || event.altKey) {
    return null;
  }
  switch (event.key) {
    case '+':
    case '=':
      return 'up';
    case '-':
    case '_':
      return 'down';
    case '0':
      return 'reset';
    default:
      return null;
  }
}

/** Last applied scale, read synchronously so startup can zoom before first render. */
export function readCachedUiScale(): number {
  try {
    const raw = window.localStorage.getItem(UI_SCALE_CACHE_KEY);
    return raw === null ? DEFAULT_UI_SCALE : normalizeUiScale(Number(raw));
  } catch {
    return DEFAULT_UI_SCALE;
  }
}

export async function applyUiScale(scale: number): Promise<void> {
  const normalized = normalizeUiScale(scale);
  try {
    window.localStorage.setItem(UI_SCALE_CACHE_KEY, String(normalized));
  } catch {
    // storage unavailable: the persisted setting still wins on next load
  }
  try {
    await getCurrentWebview().setZoom(normalized);
  } catch (error) {
    logger.warn('Failed to apply webview zoom', error);
  }
}
