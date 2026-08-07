import { useCallback, useRef, useState } from 'react';

export interface UseScrollFollowStateOptions {
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
}

export function useScrollFollowState({
  logScrollState,
}: UseScrollFollowStateOptions) {
  const [isPinned, setIsPinned] = useState(true);
  const isPinnedToBottomRef = useRef(true);
  const visualBottomRef = useRef(true);
  const shouldFollowLatestRef = useRef(true);
  const upwardReleaseDistanceRef = useRef(0);
  const selfScrollIgnoreUntilRef = useRef(0);
  // After startReached / older-page load / reached-top: stay in history mode
  // until the user is genuinely back at the latest content (or taps pin).
  // Prevents collapsed-height / top-edge false bottoms from re-arming follow.
  const isHistoryBrowsingRef = useRef(false);

  const setEffectivePinnedState = useCallback((nextPinned: boolean) => {
    isPinnedToBottomRef.current = nextPinned;
    setIsPinned(nextPinned);
  }, []);

  const enterHistoryBrowsing = useCallback(
    (reason: string) => {
      if (!isHistoryBrowsingRef.current) {
        isHistoryBrowsingRef.current = true;
        logScrollState('history-browsing:enter', { reason });
      }
    },
    [logScrollState],
  );

  const exitHistoryBrowsing = useCallback(
    (reason: string) => {
      if (!isHistoryBrowsingRef.current) {
        return;
      }
      isHistoryBrowsingRef.current = false;
      logScrollState('history-browsing:exit', { reason });
    },
    [logScrollState],
  );

  const resumeBottomFollow = useCallback(
    (reason: string) => {
      upwardReleaseDistanceRef.current = 0;
      if (shouldFollowLatestRef.current) {
        return;
      }

      shouldFollowLatestRef.current = true;
      setEffectivePinnedState(true);
      logScrollState('bottom-follow:resume', {
        reason,
      });
    },
    [logScrollState, setEffectivePinnedState],
  );

  const pauseBottomFollow = useCallback(
    (reason: string) => {
      upwardReleaseDistanceRef.current = 0;
      if (!shouldFollowLatestRef.current) {
        return;
      }

      shouldFollowLatestRef.current = false;
      setEffectivePinnedState(visualBottomRef.current);
      logScrollState('bottom-follow:pause', {
        reason,
        visualBottom: visualBottomRef.current,
      });
    },
    [logScrollState, setEffectivePinnedState],
  );

  return {
    isPinned,
    setIsPinned,
    isPinnedToBottomRef,
    visualBottomRef,
    shouldFollowLatestRef,
    upwardReleaseDistanceRef,
    selfScrollIgnoreUntilRef,
    isHistoryBrowsingRef,
    setEffectivePinnedState,
    enterHistoryBrowsing,
    exitHistoryBrowsing,
    resumeBottomFollow,
    pauseBottomFollow,
  };
}
