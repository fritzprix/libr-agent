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
  /** True while reading history / older pages — suppress auto bottom until exit. */
  isHistoryBrowsingRef: MutableRefObject<boolean>;
  /** >0 while the user is mid upward-read but before follow is fully paused. */
  upwardReleaseDistanceRef: MutableRefObject<number>;
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
  isHistoryBrowsingRef,
  upwardReleaseDistanceRef,
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

      // #1647: Virtuoso scrollToIndex(LAST) does not include Footer spacer
      // (composer overlap). Always align the footer sentinel when present so
      // the last bubble clears the composer — even after a successful Virtuoso
      // scroll.
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
      const alignmentActive = isBottomAlignmentActive(
        bottomAlignmentPhaseRef.current,
      );
      const shouldFollow = shouldFollowLatestRef.current;
      const shouldForce = isForceReason || (shouldFollow && !isNearTop);

      const shouldSuppressForPrepend =
        isPreservingPrependPositionRef.current && !isForceReason;

      // History/older-page browsing: never auto-yank to bottom (resize/stream),
      // even if follow briefly re-arms from a false visual bottom.
      const shouldSuppressForHistoryBrowsing =
        isHistoryBrowsingRef.current && !isForceReason;

      // Follow paused (history / older-page): never yank from resize/stream.
      // Do not trust visualBottom alone — prepend can false-positive bottom.
      const shouldSuppressForPausedFollow =
        !isForceReason && !alignmentActive && !shouldFollow;

      const shouldSuppressForNearTop =
        isNearTop && !isForceReason && !alignmentActive && !shouldFollow;

      // Pre-pause race: upward distance accumulating while follow still on.
      // Reject only this new request; leave any already-queued frame alone.
      const shouldSuppressForUpwardIntent =
        upwardReleaseDistanceRef.current > 0 &&
        shouldFollow &&
        !isForceReason &&
        !alignmentActive;

      if (
        shouldSuppressForPrepend ||
        shouldSuppressForHistoryBrowsing ||
        shouldSuppressForPausedFollow ||
        shouldSuppressForNearTop
      ) {
        clearScheduledAutoScroll();
        logScrollState('scheduleScrollToBottom:suppressed', {
          reason,
          shouldSuppressForPrepend,
          shouldSuppressForHistoryBrowsing,
          shouldSuppressForPausedFollow,
          shouldSuppressForNearTop,
          shouldSuppressForUpwardIntent: false,
          shouldForce,
          shouldFollow,
          upwardReleaseDistance: upwardReleaseDistanceRef.current,
        });
        return;
      }

      if (shouldSuppressForUpwardIntent) {
        logScrollState('scheduleScrollToBottom:suppressed', {
          reason,
          shouldSuppressForPrepend: false,
          shouldSuppressForHistoryBrowsing: false,
          shouldSuppressForPausedFollow: false,
          shouldSuppressForNearTop: false,
          shouldSuppressForUpwardIntent: true,
          shouldForce,
          shouldFollow,
          upwardReleaseDistance: upwardReleaseDistanceRef.current,
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
        const alignmentActiveOnFire = isBottomAlignmentActive(
          bottomAlignmentPhaseRef.current,
        );
        const shouldFollowOnFire = shouldFollowLatestRef.current;
        const shouldForceOnFire =
          isForceReasonOnFire || (shouldFollowOnFire && !isNearTopOnFire);

        // Frame-time checks are state-only. Upward noise must not cancel a
        // frame that was accepted while follow was still active; pauseBottomFollow
        // clears the queue when the user fully releases follow.
        const shouldSuppressForPrependOnFire =
          isPreservingPrependPositionRef.current && !isForceReasonOnFire;
        const shouldSuppressForHistoryBrowsingOnFire =
          isHistoryBrowsingRef.current && !isForceReasonOnFire;
        const shouldSuppressForPausedFollowOnFire =
          !isForceReasonOnFire && !alignmentActiveOnFire && !shouldFollowOnFire;
        const shouldSuppressForNearTopOnFire =
          isNearTopOnFire &&
          !isForceReasonOnFire &&
          !alignmentActiveOnFire &&
          !shouldFollowOnFire;

        if (
          shouldSuppressForPrependOnFire ||
          shouldSuppressForHistoryBrowsingOnFire ||
          shouldSuppressForPausedFollowOnFire ||
          shouldSuppressForNearTopOnFire
        ) {
          logScrollState('scheduleScrollToBottom:frame-suppressed', {
            reason,
            isPreservingPrepend: isPreservingPrependPositionRef.current,
            isHistoryBrowsing: isHistoryBrowsingRef.current,
            isNearTop: isNearTopOnFire,
            shouldFollowLatest: shouldFollowOnFire,
            visualBottom: visualBottomRef.current,
          });
          return;
        }

        logScrollState('scheduleScrollToBottom:frame-fired', {
          reason,
          shouldForce: shouldForceOnFire,
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
      isHistoryBrowsingRef,
      isPreservingPrependPositionRef,
      logScrollState,
      scheduleBottomAlignmentVerification,
      setBottomAlignmentPhase,
      shouldFollowLatestRef,
      scrollTopRef,
      upwardReleaseDistanceRef,
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
