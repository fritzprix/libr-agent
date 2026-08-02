import { useCallback, useRef, type MutableRefObject } from 'react';
import type { BottomAlignmentPhase } from '../../types';
import { isBottomAlignmentActive } from '../../utils';

export interface UseBottomAlignmentOptions {
  isPreservingPrependPositionRef: MutableRefObject<boolean>;
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
}

export function useBottomAlignment({
  isPreservingPrependPositionRef,
  logScrollState,
}: UseBottomAlignmentOptions) {
  const bottomAlignmentPhaseRef = useRef<BottomAlignmentPhase>('idle');
  const bottomAlignmentLayoutVersionRef = useRef(0);
  const bottomAlignmentRequestedVersionRef = useRef(0);
  const bottomAlignmentVerifyFrameRef = useRef<number | null>(null);

  const clearBottomAlignmentVerifyFrame = useCallback(() => {
    if (bottomAlignmentVerifyFrameRef.current !== null) {
      cancelAnimationFrame(bottomAlignmentVerifyFrameRef.current);
      bottomAlignmentVerifyFrameRef.current = null;
    }
  }, []);

  const setBottomAlignmentPhase = useCallback(
    (phase: BottomAlignmentPhase, reason: string) => {
      if (bottomAlignmentPhaseRef.current === phase) {
        return;
      }

      bottomAlignmentPhaseRef.current = phase;
      logScrollState('bottom-alignment:phase', {
        reason,
        phase,
      });
    },
    [logScrollState],
  );

  const requestBottomAlignment = useCallback(
    (reason: string) => {
      clearBottomAlignmentVerifyFrame();
      bottomAlignmentRequestedVersionRef.current =
        bottomAlignmentLayoutVersionRef.current;
      setBottomAlignmentPhase('requesting', reason);
    },
    [clearBottomAlignmentVerifyFrame, setBottomAlignmentPhase],
  );

  const abortBottomAlignment = useCallback(
    (reason: string) => {
      clearBottomAlignmentVerifyFrame();
      setBottomAlignmentPhase('aborted', reason);
    },
    [clearBottomAlignmentVerifyFrame, setBottomAlignmentPhase],
  );

  const markBottomAlignmentLayoutChanged = useCallback(
    (reason: string) => {
      if (isPreservingPrependPositionRef.current) {
        logScrollState('bottom-alignment:layout-change-skipped', {
          reason,
        });
        return;
      }

      bottomAlignmentLayoutVersionRef.current += 1;
      logScrollState('bottom-alignment:layout-change', {
        reason,
        layoutVersion: bottomAlignmentLayoutVersionRef.current,
      });
    },
    [isPreservingPrependPositionRef, logScrollState],
  );

  const maybeCompleteBottomAlignment = useCallback(
    (reason: string, visualBottom: boolean, virtuosoAtBottom: boolean) => {
      if (bottomAlignmentPhaseRef.current !== 'verifying') {
        return;
      }

      if (isPreservingPrependPositionRef.current) {
        return;
      }

      if (!visualBottom || !virtuosoAtBottom) {
        logScrollState('bottom-alignment:pending', {
          reason,
          visualBottom,
          virtuosoAtBottom,
        });
        return;
      }

      const layoutVersion = bottomAlignmentLayoutVersionRef.current;
      if (bottomAlignmentRequestedVersionRef.current !== layoutVersion) {
        bottomAlignmentRequestedVersionRef.current = layoutVersion;
        logScrollState('bottom-alignment:version-invalidated', {
          reason,
          layoutVersion,
        });
        bottomAlignmentVerifyFrameRef.current = requestAnimationFrame(() => {
          bottomAlignmentVerifyFrameRef.current = null;
          maybeCompleteBottomAlignment(
            'version-invalidated',
            visualBottom,
            virtuosoAtBottom,
          );
        });
        return;
      }

      clearBottomAlignmentVerifyFrame();
      setBottomAlignmentPhase('aligned', reason);
    },
    [
      clearBottomAlignmentVerifyFrame,
      isPreservingPrependPositionRef,
      logScrollState,
      setBottomAlignmentPhase,
    ],
  );

  const scheduleBottomAlignmentVerification = useCallback(
    (reason: string, visualBottom: boolean, virtuosoAtBottom: boolean) => {
      if (!isBottomAlignmentActive(bottomAlignmentPhaseRef.current)) {
        return;
      }

      clearBottomAlignmentVerifyFrame();
      bottomAlignmentVerifyFrameRef.current = requestAnimationFrame(() => {
        bottomAlignmentVerifyFrameRef.current = null;
        maybeCompleteBottomAlignment(reason, visualBottom, virtuosoAtBottom);
      });
    },
    [clearBottomAlignmentVerifyFrame, maybeCompleteBottomAlignment],
  );

  return {
    bottomAlignmentPhaseRef,
    bottomAlignmentLayoutVersionRef,
    bottomAlignmentRequestedVersionRef,
    bottomAlignmentVerifyFrameRef,
    clearBottomAlignmentVerifyFrame,
    setBottomAlignmentPhase,
    requestBottomAlignment,
    abortBottomAlignment,
    markBottomAlignmentLayoutChanged,
    maybeCompleteBottomAlignment,
    scheduleBottomAlignmentVerification,
  };
}
