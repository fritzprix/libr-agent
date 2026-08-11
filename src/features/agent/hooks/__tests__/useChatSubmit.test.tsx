import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useChatSubmit } from '../useChatSubmit';

const mockState = vi.hoisted(() => ({
  submit: vi.fn(),
  commitPendingFiles: vi.fn(),
  clearPendingFiles: vi.fn(),
  refetchSessionFiles: vi.fn(),
  onSubmitted: vi.fn(),
  onClearSession: vi.fn(),
  toastError: vi.fn(),
  toastSuccess: vi.fn(),
  safeInvoke: vi.fn(),
}));

vi.mock('@paralleldrive/cuid2', () => ({
  createId: () => 'msg-test',
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      maxInputContext: 10,
    },
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: mockState.toastError,
    success: mockState.toastSuccess,
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: (...args: unknown[]) => mockState.safeInvoke(...args),
}));

describe('useChatSubmit', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState.commitPendingFiles.mockResolvedValue([]);
    mockState.submit.mockResolvedValue(undefined);
    mockState.safeInvoke.mockResolvedValue({
      success: true,
      message: 'cleared',
    });
  });

  it('blocks an obvious oversize first input before submit', async () => {
    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: false,
        onSubmitted: mockState.onSubmitted,
      }),
    );

    act(() => {
      result.current.setInput('x'.repeat(21));
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockState.toastError).toHaveBeenCalledWith(
      'First input is too large to start this session. Split it up or raise Max Input Context in Settings.',
    );
    expect(mockState.submit).not.toHaveBeenCalled();
    expect(mockState.commitPendingFiles).not.toHaveBeenCalled();
    expect(result.current.input).toBe('x'.repeat(21));
  });

  it('allows large input once the session already has persisted messages', async () => {
    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: true,
      }),
    );

    act(() => {
      result.current.setInput('x'.repeat(11));
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockState.toastError).not.toHaveBeenCalled();
    expect(mockState.submit).toHaveBeenCalledTimes(1);
  });

  it('blocks non-command submit while proxy is not ready', async () => {
    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: false,
        isProxyReady: false,
      }),
    );

    act(() => {
      result.current.setInput('hello');
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockState.toastError).toHaveBeenCalledWith(
      'agent.input.proxyNotReadyToast',
    );
    expect(mockState.submit).not.toHaveBeenCalled();
    expect(result.current.input).toBe('hello');
  });

  it('allows submit once proxy is ready', async () => {
    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: false,
        isProxyReady: true,
      }),
    );

    act(() => {
      result.current.setInput('hello');
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockState.toastError).not.toHaveBeenCalled();
    expect(mockState.submit).toHaveBeenCalledTimes(1);
  });

  it('clears session history optimistically for /clear before invoke resolves', async () => {
    let resolveInvoke:
      | ((value: { success: boolean; message: string }) => void)
      | undefined;
    mockState.safeInvoke.mockImplementation(
      () =>
        new Promise<{ success: boolean; message: string }>((resolve) => {
          resolveInvoke = resolve;
        }),
    );

    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: true,
        onClearSession: mockState.onClearSession,
      }),
    );

    act(() => {
      result.current.setInput('/clear');
    });

    let submitPromise: Promise<void> | undefined;
    act(() => {
      submitPromise = result.current.handleSubmit();
    });

    expect(mockState.onClearSession).toHaveBeenCalledTimes(1);
    expect(mockState.clearPendingFiles).toHaveBeenCalledTimes(1);
    expect(mockState.safeInvoke).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveInvoke?.({ success: true, message: 'Session cleared' });
      await submitPromise;
    });

    expect(mockState.toastSuccess).toHaveBeenCalledWith('Session cleared');
    expect(mockState.onClearSession).toHaveBeenCalledTimes(1);
  });

  it('ignores nested submit while a command invoke is in flight', async () => {
    let resolveInvoke:
      | ((value: { success: boolean; message: string }) => void)
      | undefined;
    mockState.safeInvoke.mockImplementation(
      () =>
        new Promise<{ success: boolean; message: string }>((resolve) => {
          resolveInvoke = resolve;
        }),
    );

    const { result } = renderHook(() =>
      useChatSubmit({
        session: { id: 'session-1' },
        submit: mockState.submit,
        pendingFiles: [],
        commitPendingFiles: mockState.commitPendingFiles,
        clearPendingFiles: mockState.clearPendingFiles,
        refetchSessionFiles: mockState.refetchSessionFiles,
        hasPersistedMessages: true,
        onClearSession: mockState.onClearSession,
      }),
    );

    act(() => {
      result.current.setInput('/clear');
    });

    let firstSubmit: Promise<void> | undefined;
    act(() => {
      firstSubmit = result.current.handleSubmit();
    });

    act(() => {
      result.current.setInput('/clear');
    });

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(mockState.safeInvoke).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveInvoke?.({ success: true, message: 'Session cleared' });
      await firstSubmit;
    });
  });
});
