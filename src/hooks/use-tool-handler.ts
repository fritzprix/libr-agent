import { useCallback, useRef, useEffect } from 'react';

/**
 * Custom hook for creating stable tool handlers that automatically manage
 * unstable dependencies through refs to prevent infinite re-renders
 */
export function useToolHandler<
  TInput = Record<string, unknown>,
  TDeps extends object = object,
>(
  handler: (input: TInput, deps: TDeps) => void | Promise<void>,
  dependencies: TDeps,
): (input: TInput) => void | Promise<void> {
  // Create refs for all dependencies
  const depsRef = useRef<TDeps>(dependencies);

  // Update refs whenever dependencies change
  useEffect(() => {
    depsRef.current = dependencies;
  }, Object.values(dependencies));

  // Return stable handler that uses current ref values
  const stableHandler = useCallback(
    (input: TInput) => {
      try {
        return handler(input, depsRef.current);
      } catch (error) {
        console.error('Tool handler error:', error);
        throw error;
      }
    },
    [handler], // Only re-create if the handler function itself changes
  );

  return stableHandler;
}

// Even simpler - just use closure, no deps parameter needed
export function useStableHandler<T extends (...args: unknown[]) => void>(
  callback: T,
): T {
  const callbackRef = useRef<T>(callback);

  // Always use the latest callback
  useEffect(() => {
    callbackRef.current = callback;
  });

  // Return stable reference that calls the latest callback
  return useCallback(
    ((...args: unknown[]) => {
      return callbackRef.current(...args);
    }) as T,
    [],
  ); // Empty deps = always stable
}
