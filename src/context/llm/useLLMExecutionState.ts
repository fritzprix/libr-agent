import type { CompactedRange } from './types';
import { useCallback, useState } from 'react';

export function useLLMExecutionState() {
  // Tracks which sessions have an async compaction in-flight
  const [compactingMap, setCompactingMap] = useState<
    ReadonlyMap<string, boolean>
  >(new Map());
  const [compactedRangeMap, setCompactedRangeMap] = useState<
    ReadonlyMap<string, CompactedRange>
  >(new Map());
  const [awaitingCompactMap, setAwaitingCompactMap] = useState<
    ReadonlyMap<string, boolean>
  >(new Map());

  const setCompacting = useCallback((sessionId: string, value: boolean) => {
    setCompactingMap((prev) => {
      const next = new Map(prev);
      if (value) {
        next.set(sessionId, true);
      } else {
        next.delete(sessionId);
      }
      return next;
    });
  }, []);

  const setCompactedRange = useCallback(
    (sessionId: string, range: CompactedRange | undefined) => {
      setCompactedRangeMap((prev) => {
        const next = new Map(prev);
        if (range) {
          next.set(sessionId, range);
        } else {
          next.delete(sessionId);
        }
        return next;
      });
    },
    [],
  );

  const setAwaitingCompact = useCallback(
    (sessionId: string, value: boolean) => {
      setAwaitingCompactMap((prev) => {
        const next = new Map(prev);
        if (value) {
          next.set(sessionId, true);
        } else {
          next.delete(sessionId);
        }
        return next;
      });
    },
    [],
  );

  const clearSessionState = useCallback((sessionId: string) => {
    setCompactingMap((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
    setCompactedRangeMap((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
    setAwaitingCompactMap((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

  /**
   * Clears all in-memory compact-session state for ALL sessions.
   * Called when the global context strategy changes.
   */
  const clearAllCompactState = useCallback(() => {
    setCompactingMap(new Map());
    setCompactedRangeMap(new Map());
    setAwaitingCompactMap(new Map());
  }, []);

  return {
    compactingMap,
    setCompactingMap,
    compactedRangeMap,
    setCompactedRangeMap,
    awaitingCompactMap,
    setAwaitingCompactMap,
    setCompacting,
    setCompactedRange,
    setAwaitingCompact,
    clearSessionState,
    clearAllCompactState,
    isCompacting: (sessionId: string) => compactingMap.get(sessionId) === true,
    isAwaitingCompact: (sessionId: string) =>
      awaitingCompactMap.get(sessionId) === true,
    getCompactedRange: (sessionId: string) => compactedRangeMap.get(sessionId),
  };
}
