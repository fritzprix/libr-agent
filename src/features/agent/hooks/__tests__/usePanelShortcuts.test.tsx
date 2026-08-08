import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentPanelsProvider } from '@/context/AgentPanelsContext';
import {
  isEditableKeyboardTarget,
  resolvePanelShortcut,
  usePanelShortcuts,
} from '../usePanelShortcuts';

const mockTrackShortcutUsed = vi.fn();

vi.mock('@/lib/analytics', () => ({
  trackShortcutUsed: (...args: unknown[]) => mockTrackShortcutUsed(...args),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

function wrapper({ children }: { children: ReactNode }) {
  return <AgentPanelsProvider>{children}</AgentPanelsProvider>;
}

function dispatchShortcut(
  key: string,
  options: {
    metaKey?: boolean;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    altKey?: boolean;
    target?: EventTarget;
  } = {},
) {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
    metaKey: options.metaKey ?? true,
    ctrlKey: options.ctrlKey ?? false,
    shiftKey: options.shiftKey ?? true,
    altKey: options.altKey ?? false,
  });

  if (options.target) {
    Object.defineProperty(event, 'target', {
      value: options.target,
      configurable: true,
    });
  }

  document.dispatchEvent(event);
  return event;
}

describe('resolvePanelShortcut / isEditableKeyboardTarget', () => {
  it('maps Cmd+Shift+J/P/U to panels and ignores W', () => {
    expect(
      resolvePanelShortcut(
        new KeyboardEvent('keydown', {
          key: 'j',
          metaKey: true,
          shiftKey: true,
        }),
      ),
    ).toBe('processes');
    expect(
      resolvePanelShortcut(
        new KeyboardEvent('keydown', {
          key: 'p',
          metaKey: true,
          shiftKey: true,
        }),
      ),
    ).toBe('planning');
    expect(
      resolvePanelShortcut(
        new KeyboardEvent('keydown', {
          key: 'u',
          ctrlKey: true,
          shiftKey: true,
        }),
      ),
    ).toBe('workspace');
    expect(
      resolvePanelShortcut(
        new KeyboardEvent('keydown', {
          key: 'w',
          metaKey: true,
          shiftKey: true,
        }),
      ),
    ).toBeNull();
  });

  it('ignores shortcuts while focus is in an editable field', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);

    const event = new KeyboardEvent('keydown', {
      key: 'j',
      metaKey: true,
      shiftKey: true,
      bubbles: true,
    });
    Object.defineProperty(event, 'target', { value: input });

    expect(isEditableKeyboardTarget(input)).toBe(true);
    expect(resolvePanelShortcut(event)).toBeNull();

    input.remove();
  });
});

describe('usePanelShortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('opens the processes panel and tracks analytics on Cmd+Shift+J', () => {
    renderHook(() => usePanelShortcuts(), { wrapper });

    act(() => {
      dispatchShortcut('j');
    });

    expect(mockTrackShortcutUsed).toHaveBeenCalledWith(
      'processes',
      'Cmd+Shift+J',
    );
  });

  it('does not fire when typing in a textarea', () => {
    renderHook(() => usePanelShortcuts(), { wrapper });

    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);

    act(() => {
      dispatchShortcut('p', { target: textarea });
    });

    expect(mockTrackShortcutUsed).not.toHaveBeenCalled();
  });

  it('removes the listener on unmount', () => {
    const { unmount } = renderHook(() => usePanelShortcuts(), { wrapper });
    unmount();

    act(() => {
      dispatchShortcut('u');
    });

    expect(mockTrackShortcutUsed).not.toHaveBeenCalled();
  });
});
