import type { CompactedRange } from './types';
import type { CompactionPressure } from '@/models/agent-ipc';
import { useCallback, useState } from 'react';

export function useLLMExecutionState() {
  // Last post-response compaction-pressure SSOT per session for the status-bar gauge.
  const [compactionPressureMap, setCompactionPressureMap] = useState<
    ReadonlyMap<string, CompactionPressure>
  >(new Map());

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
    setCompactionPressureMap((prev) => {
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
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
    setCompactionPressureMap(new Map());
    setCompactingMap(new Map());
    setCompactedRangeMap(new Map());
    setAwaitingCompactMap(new Map());
  }, []);

  /**
   * Clears the last post-response compaction pressure after compaction completes.
   * The old pre-compaction pressure would be stale and misleading at that point.
   */
  const clearCompactionPressureForSession = useCallback((sessionId: string) => {
    setCompactionPressureMap((prev) => {
      if (!prev.has(sessionId)) return prev;
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  }, []);

  return {
    compactionPressureMap,
    setCompactionPressureMap,
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
    clearCompactionPressureForSession,
    isCompacting: (sessionId: string) => compactingMap.get(sessionId) === true,
    isAwaitingCompact: (sessionId: string) =>
      awaitingCompactMap.get(sessionId) === true,
    getCompactionPressure: (sessionId: string) =>
      compactionPressureMap.get(sessionId),
    getCompactedRange: (sessionId: string) => compactedRangeMap.get(sessionId),
  };
}
