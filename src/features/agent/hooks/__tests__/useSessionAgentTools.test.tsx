import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionRuntimeServerState } from '@/models/agent-ipc';

const mocks = vi.hoisted(() => ({
  useAgentTools: vi.fn(() => ({
    availableTools: [],
    isLoading: false,
    error: null,
  })),
  buildMcpToolsDiscoveryRevision: vi.fn(
    (_servers: SessionRuntimeServerState[], ready: boolean) =>
      ready ? 'ready:1' : 'pending:0',
  ),
  sessionId: 'session-1' as string | undefined,
  isProxyReady: false,
  servers: [] as SessionRuntimeServerState[],
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: mocks.sessionId ? { id: mocks.sessionId } : null,
    isProxyReady: mocks.isProxyReady,
    runtimeState: { servers: mocks.servers },
  }),
}));

vi.mock('@/hooks/use-agent-tools', () => ({
  buildMcpToolsDiscoveryRevision: mocks.buildMcpToolsDiscoveryRevision,
  useAgentTools: mocks.useAgentTools,
}));

import { useSessionAgentTools } from '../useSessionAgentTools';

describe('useSessionAgentTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.sessionId = 'session-1';
    mocks.isProxyReady = false;
    mocks.servers = [
      {
        name: 'stdio-slow',
        transport: 'stdio',
        status: 'connecting',
        toolCount: 0,
      },
    ];
  });

  it('passes discovery revision derived from runtime servers and proxy ready', () => {
    renderHook(() => useSessionAgentTools());

    expect(mocks.buildMcpToolsDiscoveryRevision).toHaveBeenCalledWith(
      [
        {
          name: 'stdio-slow',
          status: 'connecting',
          toolCount: 0,
        },
      ],
      false,
    );
    expect(mocks.useAgentTools).toHaveBeenCalledWith('session-1', {
      discoveryRevision: 'pending:0',
    });
  });

  it('updates revision when session becomes proxy-ready', () => {
    mocks.isProxyReady = true;
    mocks.servers = [
      {
        name: 'stdio-slow',
        transport: 'stdio',
        status: 'ready',
        toolCount: 3,
      },
    ];

    renderHook(() => useSessionAgentTools());

    expect(mocks.useAgentTools).toHaveBeenCalledWith('session-1', {
      discoveryRevision: 'ready:1',
    });
  });
});
