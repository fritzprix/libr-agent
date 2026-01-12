import { useEffect, useRef, useCallback } from 'react';

/**
 * Custom hook for debouncing function calls with proper TypeScript typing.
 * Delays execution until after the specified delay has elapsed since the last call.
 *
 * @param callback - Function to debounce
 * @param delay - Delay in milliseconds
 * @returns Debounced function and methods to cancel or flush
 *
 * @example
 * ```tsx
 * const { debounced, cancel, flush } = useDebounce(
 *   (value: string) => saveToAPI(value),
 *   500
 * );
 *
 * // Call multiple times - only last call executes after 500ms
 * const handleChange = (e) => debounced(e.target.value);
 *
 * // Cancel pending execution
 * cancel();
 *
 * // Execute immediately without waiting
 * flush();
 * ```
 */
export function useDebounce<T extends (...args: never[]) => void>(
  callback: T,
  delay: number,
): {
  debounced: (...args: Parameters<T>) => void;
  cancel: () => void;
  flush: () => void;
} {
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const callbackRef = useRef(callback);
  const argsRef = useRef<Parameters<T>>();

  // Update callback ref when callback changes
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  const cancel = useCallback(() => {
    if (timeoutRef.current !== null) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    argsRef.current = undefined;
  }, []);

  const flush = useCallback(() => {
    if (timeoutRef.current !== null) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    if (argsRef.current !== undefined) {
      const args = argsRef.current;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (callbackRef.current as any)(...args);
      argsRef.current = undefined;
    }
  }, []);

  const debounced = useCallback(
    (...args: Parameters<T>) => {
      cancel();
      argsRef.current = args;
      timeoutRef.current = setTimeout(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (callbackRef.current as any)(...args);
        timeoutRef.current = null;
        argsRef.current = undefined;
      }, delay);
    },
    [delay, cancel],
  );

  // Cleanup on unmount
  useEffect(() => {
    return cancel;
  }, [cancel]);

  return { debounced, cancel, flush };
}

/**
 * Custom hook for throttling function calls with proper TypeScript typing.
 * Ensures the function is called at most once per specified interval.
 *
 * @param callback - Function to throttle
 * @param delay - Minimum interval between calls in milliseconds
 * @param options - Configuration options
 * @returns Throttled function and methods to cancel
 *
 * @example
 * ```tsx
 * const { throttled, cancel } = useThrottle(
 *   (scrollY: number) => updateScrollPosition(scrollY),
 *   100,
 *   { leading: true, trailing: true }
 * );
 *
 * const handleScroll = () => throttled(window.scrollY);
 * ```
 */
export function useThrottleHook<T extends (...args: never[]) => void>(
  callback: T,
  delay: number,
  options: { leading?: boolean; trailing?: boolean } = {},
): {
  throttled: (...args: Parameters<T>) => void;
  cancel: () => void;
} {
  const { leading = true, trailing = true } = options;
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastCallRef = useRef<number>(0);
  const callbackRef = useRef(callback);
  const argsRef = useRef<Parameters<T>>();

  // Update callback ref when callback changes
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  const cancel = useCallback(() => {
    if (timeoutRef.current !== null) {
      clearTimeout(timeoutRef.current);
      timeoutRef.current = null;
    }
    argsRef.current = undefined;
  }, []);

  const throttled = useCallback(
    (...args: Parameters<T>) => {
      const now = Date.now();
      const timeSinceLastCall = now - lastCallRef.current;

      argsRef.current = args;

      // Leading edge
      if (leading && timeSinceLastCall >= delay) {
        lastCallRef.current = now;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (callbackRef.current as any)(...args);
        argsRef.current = undefined;
        return;
      }

      // Trailing edge
      if (trailing && timeoutRef.current === null) {
        timeoutRef.current = setTimeout(() => {
          lastCallRef.current = Date.now();
          if (argsRef.current !== undefined) {
            const args = argsRef.current;
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            (callbackRef.current as any)(...args);
            argsRef.current = undefined;
          }
          timeoutRef.current = null;
        }, delay - timeSinceLastCall);
      }
    },
    [delay, leading, trailing],
  );

  // Cleanup on unmount
  useEffect(() => {
    return cancel;
  }, [cancel]);

  return { throttled, cancel };
}
