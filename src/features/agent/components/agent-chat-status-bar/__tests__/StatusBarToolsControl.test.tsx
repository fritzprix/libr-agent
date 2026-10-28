import { act, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionRuntimeServerState } from '@/models/agent-ipc';
import { StatusBarToolsControl } from '../StatusBarToolsControl';

const mocks = vi.hoisted(() => ({
  safeInvoke: vi.fn(),
  toastError: vi.fn(),
  servers: [] as SessionRuntimeServerState[],
  sessionId: 'sess-1' as string | undefined,
}));

vi.mock('sonner', () => ({ toast: { error: mocks.toastError } }));

vi.mock('@/lib/backend/core', () => ({ safeInvoke: mocks.safeInvoke }));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: mocks.sessionId ? { id: mocks.sessionId } : null,
    runtimeState: { servers: mocks.servers },
  }),
}));

vi.mock('@/features/agent/hooks/useSessionAgentTools', () => ({
  useSessionAgentTools: () => ({
    availableTools: [],
    isLoading: false,
    error: null,
  }),
}));

vi.mock('../../AgentToolsModal', () => ({ default: () => null }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const server = (
  name: string,
  status: SessionRuntimeServerState['status'],
): SessionRuntimeServerState => ({
  name,
  transport: 'stdio',
  status,
  toolCount: 0,
});

describe('StatusBarToolsControl retry', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.safeInvoke.mockResolvedValue(null);
    mocks.sessionId = 'sess-1';
    mocks.servers = [];
  });

  it('offers no retry when no server failed', () => {
    mocks.servers = [server('ok', 'ready'), server('slow', 'timed_out')];
    render(<StatusBarToolsControl />);
    expect(
      screen.queryByTestId('retry-failed-mcp-servers'),
    ).not.toBeInTheDocument();
  });

  it('retries every failed server of an active session, not timed-out ones', async () => {
    mocks.servers = [
      server('a', 'failed'),
      server('b', 'failed'),
      server('slow', 'timed_out'),
    ];
    render(<StatusBarToolsControl />);

    await act(async () => {
      fireEvent.click(screen.getByTestId('retry-failed-mcp-servers'));
    });

    expect(mocks.safeInvoke).toHaveBeenCalledTimes(2);
    expect(mocks.safeInvoke).toHaveBeenCalledWith('retry_session_mcp_server', {
      sessionId: 'sess-1',
      serverName: 'a',
    });
    expect(mocks.safeInvoke).toHaveBeenCalledWith('retry_session_mcp_server', {
      sessionId: 'sess-1',
      serverName: 'b',
    });
  });

  it('tells the user when a retry is rejected instead of leaving it unhandled', async () => {
    mocks.servers = [server('a', 'failed')];
    mocks.safeInvoke.mockRejectedValue(new Error('no proxy for session'));
    render(<StatusBarToolsControl />);

    await act(async () => {
      fireEvent.click(screen.getByTestId('retry-failed-mcp-servers'));
    });

    expect(mocks.toastError).toHaveBeenCalledWith('no proxy for session');
  });
});
