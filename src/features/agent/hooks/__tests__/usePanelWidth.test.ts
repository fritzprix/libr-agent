import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_PANEL_WIDTH,
  MIN_PANEL_WIDTH,
  PANEL_WIDTH_STORAGE_KEY,
  clampPanelWidth,
  maxPanelWidthForContainer,
  readStoredPanelWidth,
  usePanelWidth,
  writeStoredPanelWidth,
} from '../usePanelWidth';

function createMemoryStorage() {
  const store = new Map<string, string>();
  return {
    getItem: vi.fn((key: string) => store.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store.set(key, value);
    }),
    removeItem: vi.fn((key: string) => {
      store.delete(key);
    }),
    clear: vi.fn(() => {
      store.clear();
    }),
    get store() {
      return store;
    },
  };
}

describe('clampPanelWidth / maxPanelWidthForContainer', () => {
  it('clamps to min and max', () => {
    expect(clampPanelWidth(100, 500)).toBe(MIN_PANEL_WIDTH);
    expect(clampPanelWidth(600, 500)).toBe(500);
    expect(clampPanelWidth(400.6, 500)).toBe(401);
  });

  it('uses half the container width as max', () => {
    expect(maxPanelWidthForContainer(800)).toBe(400);
    expect(maxPanelWidthForContainer(0)).toBe(DEFAULT_PANEL_WIDTH * 2);
  });
});

describe('panel width storage', () => {
  let storage: ReturnType<typeof createMemoryStorage>;

  beforeEach(() => {
    storage = createMemoryStorage();
    vi.stubGlobal('localStorage', storage);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('reads the default when storage is empty', () => {
    expect(readStoredPanelWidth()).toBe(DEFAULT_PANEL_WIDTH);
  });

  it('round-trips a stored width', () => {
    writeStoredPanelWidth(420);
    expect(storage.getItem(PANEL_WIDTH_STORAGE_KEY)).toBe('420');
    expect(readStoredPanelWidth()).toBe(420);
  });
});

describe('usePanelWidth', () => {
  let storage: ReturnType<typeof createMemoryStorage>;

  beforeEach(() => {
    storage = createMemoryStorage();
    vi.stubGlobal('localStorage', storage);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('commits width to localStorage and resets to default', () => {
    const { result } = renderHook(() => usePanelWidth());

    act(() => {
      result.current.commitPanelWidth(480);
    });
    expect(result.current.panelWidth).toBe(480);
    expect(storage.getItem(PANEL_WIDTH_STORAGE_KEY)).toBe('480');

    act(() => {
      result.current.resetPanelWidth();
    });
    expect(result.current.panelWidth).toBe(DEFAULT_PANEL_WIDTH);
    expect(storage.getItem(PANEL_WIDTH_STORAGE_KEY)).toBe(
      String(DEFAULT_PANEL_WIDTH),
    );
  });

  it('clamps live updates to the current max', () => {
    const { result } = renderHook(() => usePanelWidth());

    act(() => {
      // Before ResizeObserver measures, max is DEFAULT*2 (640).
      result.current.setPanelWidth(900);
    });
    expect(result.current.panelWidth).toBeLessThanOrEqual(
      result.current.maxWidth,
    );
    expect(result.current.panelWidth).toBeGreaterThanOrEqual(MIN_PANEL_WIDTH);
  });
});
