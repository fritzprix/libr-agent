import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentProcessPanel } from '../AgentProcessPanel';

const mockAgentCallBuiltinTool = vi.fn();

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    agentCallBuiltinTool: mockAgentCallBuiltinTool,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'session-123' },
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatState: () => ({
    messages: [],
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

const stableT = (
  key: string,
  defaultOrOptions?: string | Record<string, unknown>,
) => {
  if (typeof defaultOrOptions === 'string') {
    return defaultOrOptions;
  }
  return key;
};

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: stableT,
  }),
}));

vi.mock('@/hooks/use-agent-message-trigger', () => ({
  useAgentMessageTrigger: vi.fn(),
}));

vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: ReactNode }) => children,
  TooltipContent: ({ children }: { children: ReactNode }) => children,
}));

describe('AgentProcessPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders empty state when there are no processes', async () => {
    mockAgentCallBuiltinTool.mockResolvedValue({
      isError: false,
      structuredContent: {
        processes: [],
        total: 0,
        running: 0,
        finished: 0,
      },
    });

    render(<AgentProcessPanel isVisible />);

    await waitFor(() => {
      expect(
        screen.getByText('No background processes in this session yet.'),
      ).toBeInTheDocument();
    });

    expect(mockAgentCallBuiltinTool).toHaveBeenCalledWith(
      'session-123',
      'workspace__listProcesses',
      { statusFilter: 'all' },
    );
  });

  it('renders process rows and can request stop', async () => {
    mockAgentCallBuiltinTool.mockImplementation(
      async (_sessionId: string, toolName: string) => {
        if (toolName === 'workspace__listProcesses') {
          return {
            isError: false,
            structuredContent: {
              processes: [
                {
                  process_id: 'proc-1',
                  name: 'server',
                  command: 'pnpm tauri dev',
                  status: 'running',
                  pid: 1001,
                  started_at: '2026-08-06T00:00:00.000Z',
                  exit_code: null,
                },
              ],
              total: 1,
              running: 1,
              finished: 0,
            },
          };
        }

        if (toolName === 'workspace__stopProcess') {
          return {
            isError: false,
            content: [{ type: 'text', text: 'stopped' }],
          };
        }

        return { isError: false, structuredContent: {} };
      },
    );

    render(<AgentProcessPanel isVisible />);

    await waitFor(() => {
      expect(screen.getByText('server')).toBeInTheDocument();
    });

    expect(screen.getByText('pnpm tauri dev')).toBeInTheDocument();
    expect(screen.getByText('PID 1001')).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText('Stop process'));

    await waitFor(() => {
      expect(mockAgentCallBuiltinTool).toHaveBeenCalledWith(
        'session-123',
        'workspace__stopProcess',
        { processId: 'proc-1' },
      );
    });
  });

  it('does not fetch while hidden', async () => {
    render(<AgentProcessPanel isVisible={false} />);

    expect(screen.getByRole('region', { hidden: true })).toHaveAttribute(
      'aria-hidden',
      'true',
    );

    await waitFor(() => {
      expect(mockAgentCallBuiltinTool).not.toHaveBeenCalled();
    });
  });

  it('shows load error and retries on demand', async () => {
    mockAgentCallBuiltinTool
      .mockResolvedValueOnce({
        isError: true,
        content: [{ type: 'text', text: 'backend down' }],
      })
      .mockResolvedValueOnce({
        isError: false,
        structuredContent: {
          processes: [],
          total: 0,
          running: 0,
          finished: 0,
        },
      });

    render(<AgentProcessPanel isVisible />);

    await waitFor(() => {
      expect(screen.getByText('backend down')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

    await waitFor(() => {
      expect(
        screen.getByText('No background processes in this session yet.'),
      ).toBeInTheDocument();
    });
  });

  it('keeps output dialog open with an error banner when read fails', async () => {
    mockAgentCallBuiltinTool.mockImplementation(
      async (_sessionId: string, toolName: string) => {
        if (toolName === 'workspace__listProcesses') {
          return {
            isError: false,
            structuredContent: {
              processes: [
                {
                  process_id: 'proc-1',
                  name: 'server',
                  command: 'pnpm tauri dev',
                  status: 'running',
                  pid: 1001,
                  started_at: '2026-08-06T00:00:00.000Z',
                  exit_code: null,
                },
              ],
              total: 1,
              running: 1,
              finished: 0,
            },
          };
        }

        if (toolName === 'workspace__readProcessOutput') {
          return {
            isError: true,
            content: [{ type: 'text', text: 'output unavailable' }],
          };
        }

        return { isError: false, structuredContent: {} };
      },
    );

    render(<AgentProcessPanel isVisible />);

    await waitFor(() => {
      expect(screen.getByText('server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('Read process output'));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('output unavailable');
    });

    expect(screen.getByText('Process output')).toBeInTheDocument();
  });

  it('can poll process status from a row action', async () => {
    mockAgentCallBuiltinTool.mockImplementation(
      async (_sessionId: string, toolName: string) => {
        if (toolName === 'workspace__listProcesses') {
          return {
            isError: false,
            structuredContent: {
              processes: [
                {
                  process_id: 'proc-1',
                  name: 'server',
                  command: 'pnpm tauri dev',
                  status: 'running',
                  pid: 1001,
                  started_at: '2026-08-06T00:00:00.000Z',
                  exit_code: null,
                },
              ],
              total: 1,
              running: 1,
              finished: 0,
            },
          };
        }

        if (toolName === 'workspace__waitForProcess') {
          return {
            isError: false,
            content: [{ type: 'text', text: 'ok' }],
          };
        }

        return { isError: false, structuredContent: {} };
      },
    );

    render(<AgentProcessPanel isVisible />);

    await waitFor(() => {
      expect(screen.getByText('server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('Refresh process status'));

    await waitFor(() => {
      expect(mockAgentCallBuiltinTool).toHaveBeenCalledWith(
        'session-123',
        'workspace__waitForProcess',
        { processId: 'proc-1', timeout: 0 },
      );
    });
  });

  it('auto-refreshes process output while the dialog is open for a running process', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mockAgentCallBuiltinTool.mockImplementation(
        async (_sessionId: string, toolName: string) => {
          if (toolName === 'workspace__listProcesses') {
            return {
              isError: false,
              structuredContent: {
                processes: [
                  {
                    process_id: 'proc-1',
                    name: 'server',
                    command: 'pnpm tauri dev',
                    status: 'running',
                    pid: 1001,
                    started_at: '2026-08-06T00:00:00.000Z',
                    exit_code: null,
                  },
                ],
                total: 1,
                running: 1,
                finished: 0,
              },
            };
          }

          if (toolName === 'workspace__readProcessOutput') {
            return {
              isError: false,
              structuredContent: {
                process_id: 'proc-1',
                stream: 'both',
                mode: 'tail',
                status: 'running',
                is_process_running: true,
                outputs: {
                  stdout: { content: ['line-1'] },
                  stderr: { content: [] },
                },
              },
            };
          }

          return { isError: false, structuredContent: {} };
        },
      );

      render(<AgentProcessPanel isVisible />);

      await waitFor(() => {
        expect(screen.getByText('server')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByLabelText('Read process output'));

      await waitFor(() => {
        expect(screen.getByText('line-1')).toBeInTheDocument();
      });

      const readCallsBefore = mockAgentCallBuiltinTool.mock.calls.filter(
        (call) => call[1] === 'workspace__readProcessOutput',
      ).length;

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2500);
      });

      const readCallsAfter = mockAgentCallBuiltinTool.mock.calls.filter(
        (call) => call[1] === 'workspace__readProcessOutput',
      ).length;

      expect(readCallsAfter).toBeGreaterThan(readCallsBefore);
    } finally {
      vi.useRealTimers();
    }
  });
});
