import { renderHook } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import type { MCPContent } from '@/lib/mcp';
import type { UIActionResult } from '@mcp-ui/client';
import { useUIActionHandler } from './useUIActionHandler';

const {
  submitMock,
  executeUiTauriActionMock,
  handleUserToolCallMock,
  openExternalUrlMock,
} = vi.hoisted(() => ({
  submitMock: vi.fn(),
  executeUiTauriActionMock: vi.fn(),
  handleUserToolCallMock: vi.fn(),
  openExternalUrlMock: vi.fn(),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: submitMock,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'test-session', assistant: { id: 'test-assistant' } },
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    openExternalUrl: openExternalUrlMock,
  }),
}));

vi.mock('@/lib/backend', () => ({
  executeUiTauriAction: executeUiTauriActionMock,
  handleUserToolCall: handleUserToolCallMock,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('useUIActionHandler', () => {
  beforeEach(() => {
    submitMock.mockReset();
    executeUiTauriActionMock.mockReset();
    handleUserToolCallMock.mockReset();
    openExternalUrlMock.mockReset();
  });

  it('routes tauri tool actions through the backend command path', async () => {
    executeUiTauriActionMock.mockResolvedValue({
      success: true,
      message: 'UI Tauri action executed: tauri:downloadWorkspaceFile',
    });

    const contentRef = {
      current: [] as MCPContent[],
    };

    const { result } = renderHook(() => useUIActionHandler(contentRef));

    const action: UIActionResult = {
      type: 'tool',
      payload: {
        toolName: 'tauri:downloadWorkspaceFile',
        params: { filePath: 'notes.txt' },
      },
    };

    await expect(result.current(action)).resolves.toEqual({
      status: 'tauri-processed',
      message: 'UI Tauri action executed: tauri:downloadWorkspaceFile',
    });

    expect(executeUiTauriActionMock).toHaveBeenCalledWith(
      'test-session',
      'tauri:downloadWorkspaceFile',
      { filePath: 'notes.txt' },
    );
    expect(handleUserToolCallMock).not.toHaveBeenCalled();
    expect(submitMock).not.toHaveBeenCalled();
  });

  it('surfaces backend tauri action failures without falling back to local injection', async () => {
    executeUiTauriActionMock.mockRejectedValue(new Error('download failed'));

    const contentRef = {
      current: [] as MCPContent[],
    };

    const { result } = renderHook(() => useUIActionHandler(contentRef));

    const action: UIActionResult = {
      type: 'tool',
      payload: {
        toolName: 'tauri:downloadWorkspaceFile',
        params: { filePath: 'notes.txt' },
      },
    };

    await expect(result.current(action)).resolves.toEqual({
      status: 'error',
      message: 'download failed',
    });

    expect(executeUiTauriActionMock).toHaveBeenCalledTimes(1);
    expect(handleUserToolCallMock).not.toHaveBeenCalled();
    expect(submitMock).not.toHaveBeenCalled();
  });
});
