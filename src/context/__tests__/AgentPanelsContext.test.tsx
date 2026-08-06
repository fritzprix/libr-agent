import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';

import {
  AgentPanelsProvider,
  getSiblingPanels,
  useAgentPanels,
} from '../AgentPanelsContext';

function wrapper({ children }: { children: ReactNode }) {
  return <AgentPanelsProvider>{children}</AgentPanelsProvider>;
}

describe('AgentPanelsContext', () => {
  it('opens a panel and reports isPanelOpen', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    expect(result.current.isPanelOpen('workspace')).toBe(false);

    act(() => {
      result.current.openPanel('workspace');
    });

    expect(result.current.isPanelOpen('workspace')).toBe(true);
  });

  it('closes sibling left-side panels when opening processes', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
    });
    expect(result.current.isPanelOpen('workspace')).toBe(true);

    const before = Date.now();
    act(() => {
      result.current.openPanel('processes');
    });

    expect(result.current.isPanelOpen('processes')).toBe(true);
    expect(result.current.isPanelOpen('workspace')).toBe(false);
    expect(result.current.isPanelOpen('planning')).toBe(false);
    expect(result.current.getLastClosedAt('workspace')).toBeGreaterThanOrEqual(
      before,
    );
  });

  it('records lastClosedAt for siblings closed via toggle mutual exclusion', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
    });

    const before = Date.now();
    act(() => {
      result.current.togglePanel('processes');
    });

    expect(result.current.isPanelOpen('processes')).toBe(true);
    expect(result.current.isPanelOpen('workspace')).toBe(false);
    expect(result.current.getLastClosedAt('workspace')).toBeGreaterThanOrEqual(
      before,
    );
  });

  it('closeAllPanels closes every panel and stamps lastClosedAt', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
      result.current.openPanel('planning');
    });

    const before = Date.now();
    act(() => {
      result.current.closeAllPanels();
    });

    expect(result.current.isPanelOpen('workspace')).toBe(false);
    expect(result.current.isPanelOpen('planning')).toBe(false);
    expect(result.current.isPanelOpen('processes')).toBe(false);
    expect(result.current.getLastClosedAt('workspace')).toBeGreaterThanOrEqual(
      before,
    );
    expect(result.current.getLastClosedAt('planning')).toBeGreaterThanOrEqual(
      before,
    );
    expect(result.current.getLastClosedAt('processes')).toBeGreaterThanOrEqual(
      before,
    );
  });

  it('allows planning to stay open while a left panel opens', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('planning');
      result.current.openPanel('workspace');
    });

    expect(result.current.isPanelOpen('planning')).toBe(true);
    expect(result.current.isPanelOpen('workspace')).toBe(true);
  });

  it('records lastClosedAt when closing', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('planning');
    });

    const before = Date.now();
    act(() => {
      result.current.closePanel('planning');
    });

    expect(result.current.isPanelOpen('planning')).toBe(false);
    expect(result.current.getLastClosedAt('planning')).toBeGreaterThanOrEqual(
      before,
    );
  });

  it('records lastClosedAt when toggling closed', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
    });

    const before = Date.now();
    act(() => {
      result.current.togglePanel('workspace');
    });

    expect(result.current.isPanelOpen('workspace')).toBe(false);
    expect(result.current.getLastClosedAt('workspace')).toBeGreaterThanOrEqual(
      before,
    );
  });

  it('lists workspace and processes as siblings', () => {
    expect(getSiblingPanels('workspace')).toEqual(['processes']);
    expect(getSiblingPanels('processes')).toEqual(['workspace']);
    expect(getSiblingPanels('planning')).toEqual([]);
  });

  it('marks attention only while closed and clears it on open', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.markPanelAttention('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(true);

    act(() => {
      result.current.openPanel('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(false);

    act(() => {
      result.current.closePanel('processes');
      result.current.markPanelAttention('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(true);

    act(() => {
      result.current.togglePanel('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(false);
  });

  it('does not mark attention while the panel is already open', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('planning');
      result.current.markPanelAttention('planning');
    });

    expect(result.current.hasPanelAttention('planning')).toBe(false);
  });
});
