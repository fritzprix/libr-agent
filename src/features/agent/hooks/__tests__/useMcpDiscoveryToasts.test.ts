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

  it('shows success toast when discovery completes', () => {
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

    expect(toast.dismiss).toHaveBeenCalledWith('mcp-discovery:s1');
    expect(toast.success).toHaveBeenCalledWith(
      'agent.statusBar.mcpResultSuccess',
      expect.objectContaining({ duration: 2500 }),
    );
  });

  it('shows warning toast for partial discovery', () => {
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
      expect.objectContaining({ duration: 5000 }),
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
});
