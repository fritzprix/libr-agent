import { useCallback, useMemo, useRef, type MutableRefObject } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import {
  NEAR_TOP_SCROLL_THRESHOLD,
  SELF_SCROLL_IGNORE_WINDOW_MS,
  type BottomAlignmentPhase,
} from '../../types';
import {
  isBottomAlignmentActive,
  scrollFooterSentinelIntoView,
  scrollVirtuosoToBottom,
} from '../../utils';

export interface UseScrollSchedulerOptions {
  virtuosoRef: MutableRefObject<VirtuosoHandle | null>;
  footerEndRef: MutableRefObject<HTMLDivElement | null>;
  groupedMessageCountRef: MutableRefObject<number>;
  selfScrollIgnoreUntilRef: MutableRefObject<number>;
  shouldFollowLatestRef: MutableRefObject<boolean>;
  isPreservingPrependPositionRef: MutableRefObject<boolean>;
  bottomAlignmentPhaseRef: MutableRefObject<BottomAlignmentPhase>;
  bottomAlignmentLayoutVersionRef: MutableRefObject<number>;
  bottomAlignmentRequestedVersionRef: MutableRefObject<number>;
  visualBottomRef: MutableRefObject<boolean>;
  scrollTopRef: MutableRefObject<number>;
  setBottomAlignmentPhase: (
    phase: BottomAlignmentPhase,
    reason: string,
  ) => void;
  scheduleBottomAlignmentVerification: (
    reason: string,
    visualBottom: boolean,
    virtuosoAtBottom: boolean,
  ) => void;
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
}

export function useScrollScheduler({
  virtuosoRef,
  footerEndRef,
  groupedMessageCountRef,
  selfScrollIgnoreUntilRef,
  shouldFollowLatestRef,
  isPreservingPrependPositionRef,
  bottomAlignmentPhaseRef,
  bottomAlignmentLayoutVersionRef,
  bottomAlignmentRequestedVersionRef,
  visualBottomRef,
  scrollTopRef,
  setBottomAlignmentPhase,
  scheduleBottomAlignmentVerification,
  logScrollState,
}: UseScrollSchedulerOptions) {
  const autoScrollFrameRef = useRef<number | null>(null);

  const forceBottomScrollReasons = useMemo(
    () =>
      new Set([
        'manual-scroll-to-bottom',
        'session-changed',
        'hydrated-messages-arrived',
      ]),
    [],
  );

  const clearScheduledAutoScroll = useCallback(() => {
    if (autoScrollFrameRef.current !== null) {
      cancelAnimationFrame(autoScrollFrameRef.current);
      autoScrollFrameRef.current = null;
    }
  }, []);

  const executeScrollToBottom = useCallback(
    (reason: string): boolean => {
      const itemCount = groupedMessageCountRef.current;
      if (forceBottomScrollReasons.has(reason)) {
        selfScrollIgnoreUntilRef.current =
          performance.now() + SELF_SCROLL_IGNORE_WINDOW_MS;
      }
      logScrollState('executeScrollToBottom:start', {
        itemCount,
        reason,
        shouldFollowLatest: shouldFollowLatestRef.current,
      });

      const scrolledWithVirtuoso = scrollVirtuosoToBottom(
        virtuosoRef.current,
        itemCount,
      );

      if (scrolledWithVirtuoso) {
        logScrollState('executeScrollToBottom:virtuoso-scroll', {
          itemCount,
          reason,
          scrolledWithVirtuoso: true,
        });
      }

      if (footerEndRef.current) {
        logScrollState('executeScrollToBottom:footer-sentinel-align', {
          itemCount,
          reason,
          scrolledWithVirtuoso,
        });
        scrollFooterSentinelIntoView(footerEndRef.current);
        return true;
      }

      if (scrolledWithVirtuoso) {
        logScrollState('executeScrollToBottom:virtuoso-scroll-no-footer', {
          itemCount,
          reason,
          scrolledWithVirtuoso: true,
        });
        return true;
      }

      logScrollState('executeScrollToBottom:unavailable', {
        itemCount,
        reason,
        scrolledWithVirtuoso: false,
      });
      return false;
    },
    [
      footerEndRef,
      forceBottomScrollReasons,
      groupedMessageCountRef,
      logScrollState,
      selfScrollIgnoreUntilRef,
      shouldFollowLatestRef,
      virtuosoRef,
    ],
  );

  const scheduleScrollToBottom = useCallback(
    (reason: string, virtuosoAtBottom: boolean = false) => {
      const isForceReason = forceBottomScrollReasons.has(reason);
      const isNearTop = scrollTopRef.current <= NEAR_TOP_SCROLL_THRESHOLD;
      const shouldForce =
        isForceReason || (shouldFollowLatestRef.current && !isNearTop);
      const shouldSuppressForPrepend =
        isPreservingPrependPositionRef.current && !isForceReason;

      const shouldSuppressForUserScroll =
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !visualBottomRef.current &&
        !shouldForce;

      const shouldSuppressForNearTop =
        isNearTop && !visualBottomRef.current && !isForceReason;

      if (
        shouldSuppressForPrepend ||
        shouldSuppressForUserScroll ||
        shouldSuppressForNearTop
      ) {
        clearScheduledAutoScroll();
        logScrollState('scheduleScrollToBottom:suppressed', {
          reason,
          shouldSuppressForPrepend,
          shouldSuppressForUserScroll,
          shouldSuppressForNearTop,
          shouldForce,
        });
        return;
      }

      logScrollState('scheduleScrollToBottom', {
        reason,
      });
      clearScheduledAutoScroll();

      autoScrollFrameRef.current = requestAnimationFrame(() => {
        autoScrollFrameRef.current = null;
        const isForceReasonOnFire = forceBottomScrollReasons.has(reason);
        const isNearTopOnFire =
          scrollTopRef.current <= NEAR_TOP_SCROLL_THRESHOLD;
        const shouldForceOnFire =
          isForceReasonOnFire ||
          (shouldFollowLatestRef.current && !isNearTopOnFire);
        const shouldSuppressForPrependOnFire =
          isPreservingPrependPositionRef.current && !isForceReasonOnFire;
        const shouldSuppressForUserScrollOnFire =
          !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
          !visualBottomRef.current &&
          !shouldForceOnFire;
        const shouldSuppressForNearTopOnFire =
          isNearTopOnFire && !visualBottomRef.current && !isForceReasonOnFire;

        if (
          shouldSuppressForPrependOnFire ||
          shouldSuppressForUserScrollOnFire ||
          shouldSuppressForNearTopOnFire
        ) {
          logScrollState('scheduleScrollToBottom:frame-suppressed', {
            reason,
            isPreservingPrepend: isPreservingPrependPositionRef.current,
            isNearTop: isNearTopOnFire,
            shouldFollowLatest: shouldFollowLatestRef.current,
            visualBottom: visualBottomRef.current,
          });
          return;
        }

        logScrollState('scheduleScrollToBottom:frame-fired', {
          reason,
        });
        const didScroll = executeScrollToBottom(reason);
        if (didScroll && bottomAlignmentPhaseRef.current === 'requesting') {
          bottomAlignmentRequestedVersionRef.current =
            bottomAlignmentLayoutVersionRef.current;
          setBottomAlignmentPhase('verifying', `scroll-dispatched:${reason}`);
          scheduleBottomAlignmentVerification(
            `scroll-dispatched:${reason}`,
            visualBottomRef.current,
            virtuosoAtBottom,
          );
        }
      });
    },
    [
      bottomAlignmentLayoutVersionRef,
      bottomAlignmentPhaseRef,
      bottomAlignmentRequestedVersionRef,
      clearScheduledAutoScroll,
      executeScrollToBottom,
      forceBottomScrollReasons,
      isPreservingPrependPositionRef,
      logScrollState,
      scheduleBottomAlignmentVerification,
      setBottomAlignmentPhase,
      shouldFollowLatestRef,
      scrollTopRef,
      visualBottomRef,
    ],
  );

  return {
    autoScrollFrameRef,
    forceBottomScrollReasons,
    clearScheduledAutoScroll,
    executeScrollToBottom,
    scheduleScrollToBottom,
  };
}
