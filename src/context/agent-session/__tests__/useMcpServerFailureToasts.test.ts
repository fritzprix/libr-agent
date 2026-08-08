import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';

import type { SessionRuntimeState } from '@/models/agent-ipc';
import {
  mcpServerFailureToastId,
  useMcpServerFailureToasts,
} from '../useMcpServerFailureToasts';

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    dismiss: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

function mockRuntimeState(
  servers: SessionRuntimeState['servers'] = [],
): SessionRuntimeState {
  return {
    sequence: 1,
    phase: 'degraded',
    proxy: { exists: true, mode: 'configured', ready: true },
    initialization: { result: 'partial' },
    servers,
  };
}

describe('useMcpServerFailureToasts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('toasts server failures with deterministic toast IDs', () => {
    const state = mockRuntimeState([
      {
        name: 'exa',
        transport: 'http',
        status: 'failed',
        toolCount: 0,
        error: 'Connection refused',
      },
    ]);

    renderHook(() => useMcpServerFailureToasts('s1', state));

    const expectedId = mcpServerFailureToastId(
      's1',
      'http:exa:failed:Connection refused',
    );
    expect(toast.error).toHaveBeenCalledWith(
      'agent.statusBar.mcpServerFailed',
      expect.objectContaining({
        id: expectedId,
        duration: 8000,
      }),
    );
  });

  it('dismisses active toasts when switching sessions', () => {
    const state = mockRuntimeState([
      {
        name: 'exa',
        transport: 'http',
        status: 'failed',
        toolCount: 0,
        error: 'Connection refused',
      },
    ]);

    let sessionId = 's1';
    const { rerender } = renderHook(() =>
      useMcpServerFailureToasts(sessionId, state),
    );

    const expectedIdS1 = mcpServerFailureToastId(
      's1',
      'http:exa:failed:Connection refused',
    );
    expect(toast.error).toHaveBeenCalledWith(
      'agent.statusBar.mcpServerFailed',
      expect.objectContaining({ id: expectedIdS1 }),
    );

    // Switch to session s2
    sessionId = 's2';
    rerender();

    expect(toast.dismiss).toHaveBeenCalledWith(expectedIdS1);
  });

  it('dismisses active toasts when unmounted', () => {
    const state = mockRuntimeState([
      {
        name: 'exa',
        transport: 'http',
        status: 'failed',
        toolCount: 0,
        error: 'Connection refused',
      },
    ]);

    const { unmount } = renderHook(() =>
      useMcpServerFailureToasts('s1', state),
    );

    const expectedId = mcpServerFailureToastId(
      's1',
      'http:exa:failed:Connection refused',
    );
    unmount();

    expect(toast.dismiss).toHaveBeenCalledWith(expectedId);
  });

  it('does not duplicate toasts when switching back to a previously toasted session', () => {
    const state = mockRuntimeState([
      {
        name: 'exa',
        transport: 'http',
        status: 'failed',
        toolCount: 0,
        error: 'Connection refused',
      },
    ]);

    let sessionId = 's1';
    const { rerender } = renderHook(() =>
      useMcpServerFailureToasts(sessionId, state),
    );

    expect(toast.error).toHaveBeenCalledTimes(1);

    // Switch to session s2
    sessionId = 's2';
    rerender();

    vi.clearAllMocks();

    // Switch back to s1
    sessionId = 's1';
    rerender();

    // Should not trigger a new toast because s1 already toasted 'exa:failed'
    expect(toast.error).not.toHaveBeenCalled();
  });

  it('toasts server timeouts when status is timed_out', () => {
    const state = mockRuntimeState([
      {
        name: 'slow-stdio',
        transport: 'stdio',
        status: 'timed_out',
        toolCount: 0,
        error: 'Tool discovery timed out after 30s',
      },
    ]);

    renderHook(() => useMcpServerFailureToasts('s1', state));

    const expectedId = mcpServerFailureToastId(
      's1',
      'stdio:slow-stdio:timed_out:Tool discovery timed out after 30s',
    );
    expect(toast.error).toHaveBeenCalledWith(
      'agent.statusBar.mcpServerTimeout',
      expect.objectContaining({
        id: expectedId,
        duration: 8000,
      }),
    );
  });
});
