import { describe, expect, it } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';

import { AgentPanelsProvider, useAgentPanels } from '../AgentPanelsContext';

function wrapper({ children }: { children: ReactNode }) {
  return <AgentPanelsProvider>{children}</AgentPanelsProvider>;
}

describe('AgentPanelsContext', () => {
  it('opens a panel tab and reports isPanelOpen only for the active tab', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    expect(result.current.isShellOpen()).toBe(false);
    expect(result.current.isPanelOpen('workspace')).toBe(false);

    act(() => {
      result.current.openPanel('workspace');
    });

    expect(result.current.isShellOpen()).toBe(true);
    expect(result.current.activeTab).toBe('workspace');
    expect(result.current.isPanelOpen('workspace')).toBe(true);
    expect(result.current.isPanelOpen('processes')).toBe(false);
  });

  it('switches tabs without closing the shell', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
    });
    act(() => {
      result.current.openPanel('processes');
    });

    expect(result.current.isShellOpen()).toBe(true);
    expect(result.current.activeTab).toBe('processes');
    expect(result.current.isPanelOpen('processes')).toBe(true);
    expect(result.current.isPanelOpen('workspace')).toBe(false);
  });

  it('togglePanel closes when the same tab is active, otherwise opens that tab', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.togglePanel('planning');
    });
    expect(result.current.isPanelOpen('planning')).toBe(true);

    act(() => {
      result.current.togglePanel('processes');
    });
    expect(result.current.isShellOpen()).toBe(true);
    expect(result.current.isPanelOpen('processes')).toBe(true);

    const before = Date.now();
    act(() => {
      result.current.togglePanel('processes');
    });
    expect(result.current.isShellOpen()).toBe(false);
    expect(result.current.getLastClosedAt()).toBeGreaterThanOrEqual(before);
  });

  it('closeAllPanels closes the shell and clears attention', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
      result.current.markPanelAttention('planning');
    });

    const before = Date.now();
    act(() => {
      result.current.closeAllPanels();
    });

    expect(result.current.isShellOpen()).toBe(false);
    expect(result.current.getLastClosedAt()).toBeGreaterThanOrEqual(before);
    expect(result.current.hasPanelAttention('planning')).toBe(false);
  });

  it('marks attention only when the tab is not currently visible', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.openPanel('workspace');
      result.current.markPanelAttention('workspace');
    });
    expect(result.current.hasPanelAttention('workspace')).toBe(false);

    act(() => {
      result.current.markPanelAttention('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(true);

    act(() => {
      result.current.closeShell();
      result.current.markPanelAttention('planning');
    });
    expect(result.current.hasPanelAttention('planning')).toBe(true);
  });

  it('clears attention when opening a tab', () => {
    const { result } = renderHook(() => useAgentPanels(), { wrapper });

    act(() => {
      result.current.markPanelAttention('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(true);

    act(() => {
      result.current.openPanel('processes');
    });
    expect(result.current.hasPanelAttention('processes')).toBe(false);
  });
});
