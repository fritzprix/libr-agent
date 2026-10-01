import { useSyncExternalStore } from 'react';

type HighlightState = {
  name: string | null;
  clearAt: number | null;
};

let highlightState: HighlightState = { name: null, clearAt: null };
const listeners = new Set<() => void>();
let clearTimer: ReturnType<typeof setTimeout> | null = null;

function emit(): void {
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): string | null {
  return highlightState.name;
}

function getServerSnapshot(): string | null {
  return null;
}

/**
 * Highlight a recommended preset card (by preset name) for filming.
 * Pass `ms` to auto-clear; omit or 0 to clear only on next highlight/navigate.
 */
export function setPresetHighlight(name: string | null, ms?: number): void {
  if (clearTimer) {
    clearTimeout(clearTimer);
    clearTimer = null;
  }

  if (!name) {
    highlightState = { name: null, clearAt: null };
    emit();
    return;
  }

  const duration = typeof ms === 'number' && ms > 0 ? ms : undefined;
  highlightState = {
    name,
    clearAt: duration ? Date.now() + duration : null,
  };
  emit();

  if (duration) {
    clearTimer = setTimeout(() => {
      highlightState = { name: null, clearAt: null };
      clearTimer = null;
      emit();
    }, duration);
  }
}

export function usePresetHighlightName(): string | null {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
