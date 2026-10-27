import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { safeInvoke } from '@/lib/backend/core';
import type { SessionRuntimeServerState } from '@/models/agent-ipc';
import { SessionLoadingOverlay } from '../SessionLoadingOverlay';

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn().mockResolvedValue(null),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('SessionLoadingOverlay', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders servers list and shows retry button for failed servers when sessionId is provided', async () => {
    const servers: SessionRuntimeServerState[] = [
      {
        name: 'test-server',
        transport: 'stdio',
        status: 'failed',
        toolCount: 0,
        error: 'Process exited with code 1',
      },
      {
        name: 'ready-server',
        transport: 'http',
        status: 'ready',
        toolCount: 5,
      },
    ];

    render(
      <SessionLoadingOverlay
        label="Loading session"
        sessionId="sess-123"
        variant="blocking"
        servers={servers}
      />,
    );

    expect(screen.getByText('test-server')).toBeInTheDocument();
    expect(screen.getByText('ready-server')).toBeInTheDocument();

    const retryButton = screen.getByRole('button', { name: /common\.retry/i });
    expect(retryButton).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(retryButton);
    });

    expect(safeInvoke).toHaveBeenCalledWith('retry_session_mcp_server', {
      sessionId: 'sess-123',
      serverName: 'test-server',
    });
  });

  it('does not render retry button if sessionId is not provided', () => {
    const servers: SessionRuntimeServerState[] = [
      {
        name: 'test-server',
        transport: 'stdio',
        status: 'failed',
        toolCount: 0,
        error: 'Process exited with code 1',
      },
    ];

    render(
      <SessionLoadingOverlay
        label="Loading session"
        variant="blocking"
        servers={servers}
      />,
    );

    expect(screen.getByText('test-server')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /common\.retry/i }),
    ).not.toBeInTheDocument();
  });
});
