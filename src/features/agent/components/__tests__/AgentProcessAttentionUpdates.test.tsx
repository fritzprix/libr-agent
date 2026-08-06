import { act, render, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentProcessAttentionUpdates } from '../AgentProcessAttentionUpdates';

const mockAgentCallBuiltinTool = vi.fn();
const mockMarkPanelAttention = vi.fn();
let panelOpen = false;

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

vi.mock('@/context/AgentPanelsContext', () => ({
  useAgentPanels: () => ({
    isPanelOpen: (id: string) => id === 'processes' && panelOpen,
    markPanelAttention: mockMarkPanelAttention,
  }),
}));

vi.mock('@/hooks/use-agent-message-trigger', () => ({
  useAgentMessageTrigger: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

function listPayload(
  processes: Array<{
    process_id: string;
    status: string;
    command?: string;
  }>,
) {
  return {
    isError: false,
    structuredContent: {
      processes: processes.map((process) => ({
        process_id: process.process_id,
        name: null,
        command: process.command ?? 'echo',
        status: process.status,
        pid: 1,
        started_at: '2026-08-06T00:00:00.000Z',
        exit_code: null,
      })),
      total: processes.length,
      running: processes.filter(
        (process) =>
          process.status === 'running' || process.status === 'starting',
      ).length,
      finished: processes.filter(
        (process) =>
          process.status !== 'running' && process.status !== 'starting',
      ).length,
    },
  };
}

describe('AgentProcessAttentionUpdates', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    panelOpen = false;
  });

  it('does not mark attention while the processes panel is open', async () => {
    panelOpen = true;
    mockAgentCallBuiltinTool.mockResolvedValue(
      listPayload([{ process_id: 'proc-1', status: 'running' }]),
    );

    render(<AgentProcessAttentionUpdates />);

    await waitFor(() => {
      expect(mockAgentCallBuiltinTool).not.toHaveBeenCalled();
    });
    expect(mockMarkPanelAttention).not.toHaveBeenCalled();
  });

  it('marks attention on subsequent poll when status changes while closed', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      mockAgentCallBuiltinTool
        .mockResolvedValueOnce(
          listPayload([{ process_id: 'proc-1', status: 'running' }]),
        )
        .mockResolvedValueOnce(
          listPayload([{ process_id: 'proc-1', status: 'finished' }]),
        );

      render(<AgentProcessAttentionUpdates />);

      await act(async () => {
        await Promise.resolve();
      });
      expect(mockAgentCallBuiltinTool).toHaveBeenCalledTimes(1);
      expect(mockMarkPanelAttention).not.toHaveBeenCalled();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2500);
      });

      expect(mockAgentCallBuiltinTool).toHaveBeenCalledTimes(2);
      expect(mockMarkPanelAttention).toHaveBeenCalledWith('processes');
    } finally {
      vi.useRealTimers();
    }
  });
});
