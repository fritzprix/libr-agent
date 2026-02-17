import { useRef } from 'react';

/**
 * Returns a stable array reference if the contents (shallow equality) match the previous render.
 * Useful for preventing re-renders when a new array is created with the same items.
 */
export function useStableArray<T>(array: T[]): T[] {
  const ref = useRef(array);

  if (
    array.length !== ref.current.length ||
    array.some((item, i) => item !== ref.current[i])
  ) {
    ref.current = array;
  }

  return ref.current;
}
