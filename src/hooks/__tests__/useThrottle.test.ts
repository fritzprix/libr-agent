import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { useThrottle } from '../useThrottle';

describe('useThrottle', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('should call the callback immediately on first invocation', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100));

    act(() => {
      result.current();
    });

    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('should throttle subsequent calls within the delay period', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100));

    act(() => {
      result.current();
      result.current();
      result.current();
    });

    // Only the first call should execute immediately
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance time by 100ms
    act(() => {
      vi.advanceTimersByTime(100);
    });

    // The last call should execute after the delay
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it('should allow calls after the delay period', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100));

    act(() => {
      result.current();
    });

    expect(callback).toHaveBeenCalledTimes(1);

    // Advance time past the delay
    act(() => {
      vi.advanceTimersByTime(100);
    });

    act(() => {
      result.current();
    });

    expect(callback).toHaveBeenCalledTimes(2);
  });

  it('should pass arguments to the throttled callback', () => {
    const callback = vi.fn();
    const { result } = renderHook(() =>
      useThrottle((...args: unknown[]) => callback(...args), 100),
    );

    act(() => {
      result.current(42, 'test');
    });

    expect(callback).toHaveBeenCalledWith(42, 'test');
  });

  it('should cancel pending calls on unmount', () => {
    const callback = vi.fn();
    const { result, unmount } = renderHook(() => useThrottle(callback, 100));

    act(() => {
      result.current();
      result.current(); // This will be scheduled
    });

    expect(callback).toHaveBeenCalledTimes(1);

    // Unmount before the scheduled call executes
    unmount();

    // Advance time
    act(() => {
      vi.advanceTimersByTime(100);
    });

    // Callback should not be called again after unmount
    expect(callback).toHaveBeenCalledTimes(1);
  });

  it('should update the callback when it changes', () => {
    const callback1 = vi.fn();
    const callback2 = vi.fn();

    const { result, rerender } = renderHook(
      ({ cb, delay }) => useThrottle(cb, delay),
      {
        initialProps: { cb: callback1, delay: 100 },
      },
    );

    act(() => {
      result.current();
    });

    expect(callback1).toHaveBeenCalledTimes(1);
    expect(callback2).toHaveBeenCalledTimes(0);

    // Change the callback
    rerender({ cb: callback2, delay: 100 });

    // Advance time and call again
    act(() => {
      vi.advanceTimersByTime(100);
    });

    act(() => {
      result.current();
    });

    expect(callback1).toHaveBeenCalledTimes(1);
    expect(callback2).toHaveBeenCalledTimes(1);
  });

  it('should handle rapid successive calls correctly', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 100));

    // Call 10 times rapidly
    act(() => {
      for (let i = 0; i < 10; i++) {
        result.current();
      }
    });

    // Only the first call should execute immediately
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance time
    act(() => {
      vi.advanceTimersByTime(100);
    });

    // The last call should execute
    expect(callback).toHaveBeenCalledTimes(2);
  });

  it('should work with different delay values', () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useThrottle(callback, 500));

    act(() => {
      result.current();
    });

    expect(callback).toHaveBeenCalledTimes(1);

    // Advance time by less than delay
    act(() => {
      vi.advanceTimersByTime(300);
    });

    act(() => {
      result.current();
    });

    // Should not call yet
    expect(callback).toHaveBeenCalledTimes(1);

    // Advance remaining time
    act(() => {
      vi.advanceTimersByTime(200);
    });

    // Now the scheduled call should execute
    expect(callback).toHaveBeenCalledTimes(2);
  });
});
