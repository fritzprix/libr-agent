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

  const setEffectivePinnedState = useCallback((nextPinned: boolean) => {
    isPinnedToBottomRef.current = nextPinned;
    setIsPinned(nextPinned);
  }, []);

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
    setEffectivePinnedState,
    resumeBottomFollow,
    pauseBottomFollow,
  };
}
