import type { CompactedRange } from './types';
import { useCallback, useRef, useState } from 'react';
import { toast } from 'sonner';
import { compactSessionToastId } from './compact-toast-id';

export { compactSessionToastId } from './compact-toast-id';

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
  // Sessions that may still show a `duration: Infinity` compact loading toast.
  const compactToastSessionsRef = useRef(new Set<string>());

  const setCompacting = useCallback((sessionId: string, value: boolean) => {
    if (value) {
      compactToastSessionsRef.current.add(sessionId);
    } else {
      compactToastSessionsRef.current.delete(sessionId);
    }
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
    // Delete/reset never emit SUCCEEDED/FAILED; dismiss the immortal loading toast.
    toast.dismiss(compactSessionToastId(sessionId));
    compactToastSessionsRef.current.delete(sessionId);
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
    for (const sessionId of compactToastSessionsRef.current) {
      toast.dismiss(compactSessionToastId(sessionId));
    }
    compactToastSessionsRef.current.clear();
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
