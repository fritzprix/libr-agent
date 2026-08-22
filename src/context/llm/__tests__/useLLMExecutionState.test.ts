import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';

import {
  compactSessionToastId,
  useLLMExecutionState,
} from '../useLLMExecutionState';

vi.mock('sonner', () => ({
  toast: {
    dismiss: vi.fn(),
  },
}));

describe('useLLMExecutionState compact toast cleanup', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('dismisses compact-${sessionId} toast when clearSessionState runs', () => {
    const { result } = renderHook(() => useLLMExecutionState());

    act(() => {
      result.current.setCompacting('session-a', true);
    });

    act(() => {
      result.current.clearSessionState('session-a');
    });

    expect(toast.dismiss).toHaveBeenCalledWith(
      compactSessionToastId('session-a'),
    );
    expect(result.current.isCompacting('session-a')).toBe(false);
  });

  it('dismisses all tracked compact toasts when clearAllCompactState runs', () => {
    const { result } = renderHook(() => useLLMExecutionState());

    act(() => {
      result.current.setCompacting('session-a', true);
      result.current.setCompacting('session-b', true);
    });

    act(() => {
      result.current.clearAllCompactState();
    });

    expect(toast.dismiss).toHaveBeenCalledWith(
      compactSessionToastId('session-a'),
    );
    expect(toast.dismiss).toHaveBeenCalledWith(
      compactSessionToastId('session-b'),
    );
    expect(result.current.isCompacting('session-a')).toBe(false);
    expect(result.current.isCompacting('session-b')).toBe(false);
  });
});
