import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { useDebounce } from '../useDebounce';

describe('useDebounce', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('should not call the callback immediately', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test');

    expect(callback).not.toHaveBeenCalled();
  });

  it('should call the callback after the specified delay', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test');

    act(() => {
      vi.advanceTimersByTime(499);
    });
    expect(callback).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(callback).toHaveBeenCalledWith('test');
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('should reset the timer if called again before the delay expires', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test1');

    act(() => {
      vi.advanceTimersByTime(250);
    });

    result.current.debounced('test2');

    act(() => {
      vi.advanceTimersByTime(250);
    });
    expect(callback).not.toHaveBeenCalled(); // Original timer was reset

    act(() => {
      vi.advanceTimersByTime(250);
    });
    expect(callback).toHaveBeenCalledWith('test2');
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('should correctly use the latest callback if it changes', () => {
    const callback1 = vi.fn();
    const callback2 = vi.fn();

    const { result, rerender } = renderHook(
      ({ cb }) => useDebounce(cb, 500),
      { initialProps: { cb: callback1 } }
    );

    result.current.debounced('test');

    // Re-render with new callback
    rerender({ cb: callback2 });

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(callback1).not.toHaveBeenCalled();
    expect(callback2).toHaveBeenCalledWith('test');
    expect(callback2).toHaveBeenCalledTimes(1);
  });

  it('should cancel the pending timeout when cancel is called', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test');

    act(() => {
      result.current.cancel();
      vi.advanceTimersByTime(500);
    });

    expect(callback).not.toHaveBeenCalled();
  });

  it('should ignore flush if there is no pending timeout', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    act(() => {
      result.current.flush();
    });

    expect(callback).not.toHaveBeenCalled();
  });

  it('should execute the callback immediately and cancel the timeout when flush is called', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test');

    act(() => {
      result.current.flush();
    });

    expect(callback).toHaveBeenCalledWith('test');
    expect(callback).toHaveBeenCalledTimes(1);

    // Ensure timeout is cleared
    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(callback).toHaveBeenCalledTimes(1); // No additional call
  });

  it('should cancel the timeout on unmount', () => {
    const callback = vi.fn();
    const { result, unmount } = renderHook(() => useDebounce(callback, 500));

    result.current.debounced('test');

    unmount();

    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(callback).not.toHaveBeenCalled();
  });
});
