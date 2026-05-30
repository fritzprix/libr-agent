import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useChatSubmit } from '../useChatSubmit';

const mockState = vi.hoisted(() => ({
  submit: vi.fn(),
  commitPendingFiles: vi.fn(),
  clearPendingFiles: vi.fn(),
  refetchSessionFiles: vi.fn(),
  onSubmitted: vi.fn(),
  toastError: vi.fn(),
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
  },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('useChatSubmit', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState.commitPendingFiles.mockResolvedValue([]);
    mockState.submit.mockResolvedValue(undefined);
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
});
