/**
 * Desktop agent side-panel width: clamp, persist, and container-relative max.
 * Rail overlays the chat body — chat column width never changes.
 */

import { useCallback, useEffect, useRef, useState } from 'react';

export const DEFAULT_PANEL_WIDTH = 320;
export const MIN_PANEL_WIDTH = 320;
export const PANEL_WIDTH_MAX_RATIO = 0.5;
export const PANEL_WIDTH_STORAGE_KEY = 'libragent:agent-panel-width:v1';

export function clampPanelWidth(width: number, maxWidth: number): number {
  const safeMax = Math.max(MIN_PANEL_WIDTH, Math.floor(maxWidth));
  return Math.min(Math.max(Math.round(width), MIN_PANEL_WIDTH), safeMax);
}

export function readStoredPanelWidth(): number {
  try {
    const raw = localStorage.getItem(PANEL_WIDTH_STORAGE_KEY);
    if (raw === null) {
      return DEFAULT_PANEL_WIDTH;
    }
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
      return DEFAULT_PANEL_WIDTH;
    }
    return clampPanelWidth(parsed, Number.POSITIVE_INFINITY);
  } catch {
    return DEFAULT_PANEL_WIDTH;
  }
}

export function writeStoredPanelWidth(width: number): void {
  try {
    localStorage.setItem(PANEL_WIDTH_STORAGE_KEY, String(width));
  } catch {
    // Private mode / quota — ignore.
  }
}

export function maxPanelWidthForContainer(containerWidth: number): number {
  if (containerWidth <= 0) {
    return DEFAULT_PANEL_WIDTH * 2;
  }
  return Math.max(
    MIN_PANEL_WIDTH,
    Math.floor(containerWidth * PANEL_WIDTH_MAX_RATIO),
  );
}

export function usePanelWidth() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerWidth, setContainerWidth] = useState(0);
  const [panelWidth, setPanelWidthState] = useState(readStoredPanelWidth);

  const maxWidth = maxPanelWidthForContainer(containerWidth);

  useEffect(() => {
    const node = containerRef.current;
    if (!node || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) {
        return;
      }
      setContainerWidth(Math.floor(entry.contentRect.width));
    });

    observer.observe(node);
    setContainerWidth(Math.floor(node.getBoundingClientRect().width));

    return () => {
      observer.disconnect();
    };
  }, []);

  useEffect(() => {
    setPanelWidthState((current) => {
      const next = clampPanelWidth(current, maxWidth);
      if (next !== current) {
        writeStoredPanelWidth(next);
      }
      return next === current ? current : next;
    });
  }, [maxWidth]);

  const setPanelWidth = useCallback(
    (next: number) => {
      setPanelWidthState(clampPanelWidth(next, maxWidth));
    },
    [maxWidth],
  );

  const commitPanelWidth = useCallback(
    (next?: number) => {
      setPanelWidthState((current) => {
        const value = clampPanelWidth(next ?? current, maxWidth);
        writeStoredPanelWidth(value);
        return value;
      });
    },
    [maxWidth],
  );

  const resetPanelWidth = useCallback(() => {
    const value = clampPanelWidth(DEFAULT_PANEL_WIDTH, maxWidth);
    setPanelWidthState(value);
    writeStoredPanelWidth(value);
  }, [maxWidth]);

  return {
    containerRef,
    panelWidth,
    maxWidth,
    setPanelWidth,
    commitPanelWidth,
    resetPanelWidth,
  };
}
