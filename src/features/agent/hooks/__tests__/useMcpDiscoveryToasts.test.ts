import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';

import type { SessionRuntimeServerState } from '@/models/agent-ipc';
import { useMcpDiscoveryToasts } from '../useMcpDiscoveryToasts';

vi.mock('sonner', () => ({
  toast: {
    loading: vi.fn(),
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
    dismiss: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

const readyServers: SessionRuntimeServerState[] = [
  {
    name: 'exa',
    transport: 'http',
    status: 'ready',
    toolCount: 2,
  },
];

describe('useMcpDiscoveryToasts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading toast while proxy is not ready', () => {
    renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: false,
        phase: 'initializing',
        initResult: 'pending',
        servers: readyServers,
        sessionId: 's1',
        currentStep: 'Loading MCP: exa (0/1)',
      }),
    );

    expect(toast.loading).toHaveBeenCalledWith('Loading MCP: exa (0/1)', {
      id: 'mcp-discovery:s1',
    });
  });

  it('shows success toast with deterministic ID when discovery completes', () => {
    renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: true,
        phase: 'ready',
        initResult: 'success',
        servers: readyServers,
        sessionId: 's1',
        currentStep: 'MCP ready: exa',
      }),
    );

    expect(toast.success).toHaveBeenCalledWith(
      'agent.statusBar.mcpResultSuccess',
      expect.objectContaining({ id: 'mcp-discovery:s1', duration: 2500 }),
    );
  });

  it('dismisses loading toast when builtin-only session becomes ready', () => {
    const { rerender } = renderHook(
      ({
        isProxyReady,
        phase,
        initResult,
      }: {
        isProxyReady: boolean;
        phase: 'hydrating' | 'ready';
        initResult: 'pending' | 'success';
      }) =>
        useMcpDiscoveryToasts({
          hasSession: true,
          isProxyReady,
          phase,
          initResult,
          servers: [],
          sessionId: 's-builtin',
          currentStep: 'Starting session...',
        }),
      {
        initialProps: {
          isProxyReady: false,
          phase: 'hydrating' as const,
          initResult: 'pending' as const,
        },
      },
    );

    expect(toast.loading).toHaveBeenCalledWith('Starting session...', {
      id: 'mcp-discovery:s-builtin',
    });
    vi.clearAllMocks();

    rerender({
      isProxyReady: true,
      phase: 'ready',
      initResult: 'success',
    });

    expect(toast.dismiss).toHaveBeenCalledWith('mcp-discovery:s-builtin');
    expect(toast.success).not.toHaveBeenCalled();
    expect(toast.warning).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it('shows warning toast with deterministic ID for partial discovery', () => {
    renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: true,
        phase: 'degraded',
        initResult: 'partial',
        servers: [
          {
            name: 'slow-stdio',
            transport: 'stdio',
            status: 'timed_out',
            toolCount: 0,
            error: 'Tool discovery timed out',
          },
          {
            name: 'http-ok',
            transport: 'http',
            status: 'ready',
            toolCount: 3,
          },
        ],
        sessionId: 's-timeout',
      }),
    );

    expect(toast.warning).toHaveBeenCalledWith(
      'agent.statusBar.mcpResultPartial',
      expect.objectContaining({ id: 'mcp-discovery:s-timeout', duration: 5000 }),
    );
  });

  it('skips failed summary toast when per-server feedback exists', () => {
    renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: true,
        phase: 'failed',
        initResult: 'failed',
        servers: [
          {
            name: 'broken',
            transport: 'stdio',
            status: 'failed',
            toolCount: 0,
            error: 'boom',
          },
        ],
        sessionId: 's-fail',
      }),
    );

    expect(toast.error).not.toHaveBeenCalled();
  });

  it('shows failed summary toast with deterministic ID when no per-server feedback exists', () => {
    renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: true,
        phase: 'failed',
        initResult: 'failed',
        servers: [
          {
            name: 'configured-stdio',
            transport: 'stdio',
            status: 'ready',
            toolCount: 0,
          },
        ],
        sessionId: 's-fail-summary',
      }),
    );

    expect(toast.error).toHaveBeenCalledWith(
      'agent.statusBar.mcpResultFailed',
      expect.objectContaining({
        id: 'mcp-discovery:s-fail-summary',
        duration: 8000,
      }),
    );
  });

  it('dismisses discovery toast on unmount', () => {
    const { unmount } = renderHook(() =>
      useMcpDiscoveryToasts({
        hasSession: true,
        isProxyReady: false,
        phase: 'initializing',
        initResult: 'pending',
        servers: readyServers,
        sessionId: 's1',
      }),
    );

    vi.clearAllMocks();
    unmount();

    expect(toast.dismiss).toHaveBeenCalledWith('mcp-discovery:s1');
  });

  it('does not re-trigger result toast when switching back to an already-ready session', () => {
    let currentSessionId = 's1';
    const props = (sessionId: string) => ({
      hasSession: true,
      isProxyReady: true,
      phase: 'ready' as const,
      initResult: 'success' as const,
      servers: readyServers,
      sessionId,
    });

    const { rerender } = renderHook(() =>
      useMcpDiscoveryToasts(props(currentSessionId)),
    );

    expect(toast.success).toHaveBeenCalledTimes(1);

    // Switch to s2
    currentSessionId = 's2';
    rerender();
    expect(toast.success).toHaveBeenCalledTimes(2);

    vi.clearAllMocks();

    // Switch back to s1
    currentSessionId = 's1';
    rerender();

    // Should not re-fire toast.success for s1
    expect(toast.success).not.toHaveBeenCalled();
  });
});
