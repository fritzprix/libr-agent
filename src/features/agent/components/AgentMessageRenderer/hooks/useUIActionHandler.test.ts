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
  loggerInfoMock,
  loggerWarnMock,
  loggerErrorMock,
  loggerDebugMock,
} = vi.hoisted(() => ({
  submitMock: vi.fn(),
  executeUiTauriActionMock: vi.fn(),
  handleUserToolCallMock: vi.fn(),
  openExternalUrlMock: vi.fn(),
  loggerInfoMock: vi.fn(),
  loggerWarnMock: vi.fn(),
  loggerErrorMock: vi.fn(),
  loggerDebugMock: vi.fn(),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: submitMock,
  }),
}));

const sessionStateMock = vi.hoisted(() => ({
  session: {
    id: 'test-session',
    assistant: { id: 'test-assistant' },
  } as { id: string; assistant: { id: string } } | null,
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: sessionStateMock.session,
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
    info: loggerInfoMock,
    warn: loggerWarnMock,
    error: loggerErrorMock,
    debug: loggerDebugMock,
  }),
}));

describe('useUIActionHandler', () => {
  beforeEach(() => {
    sessionStateMock.session = {
      id: 'test-session',
      assistant: { id: 'test-assistant' },
    };
    submitMock.mockReset();
    executeUiTauriActionMock.mockReset();
    handleUserToolCallMock.mockReset();
    openExternalUrlMock.mockReset();
    loggerInfoMock.mockReset();
    loggerWarnMock.mockReset();
    loggerErrorMock.mockReset();
    loggerDebugMock.mockReset();
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

  it('falls back to message session id when context session is missing', async () => {
    sessionStateMock.session = null;
    handleUserToolCallMock.mockResolvedValue(undefined);

    const contentRef = {
      current: [] as MCPContent[],
    };

    const { result } = renderHook(() =>
      useUIActionHandler(contentRef, 'message-session'),
    );

    const action: UIActionResult = {
      type: 'tool',
      payload: {
        toolName: 'workspace__export',
        params: { path: 'out.md' },
      },
    };

    await expect(result.current(action)).resolves.toEqual({
      status: 'tool-submitted',
      tool: 'workspace__export',
    });

    expect(handleUserToolCallMock).toHaveBeenCalledWith(
      'message-session',
      'workspace__export',
      { path: 'out.md' },
    );
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

  it('ignores stale workflow-cancelled tool results without error logging', async () => {
    handleUserToolCallMock.mockRejectedValue(
      new Error('UI tool result orphaned (workflow inactive)'),
    );

    const contentRef = {
      current: [] as MCPContent[],
    };

    const { result } = renderHook(() => useUIActionHandler(contentRef));

    const action: UIActionResult = {
      type: 'tool',
      payload: {
        toolName: 'getUserAnswer',
        params: { messageId: 'msg-1', answer: 'Option A' },
      },
    };

    await expect(result.current(action)).resolves.toEqual({
      status: 'ignored',
      message: 'UI tool result orphaned (workflow inactive)',
    });

    expect(loggerErrorMock).not.toHaveBeenCalled();
    expect(loggerInfoMock).toHaveBeenCalled();
  });
});
