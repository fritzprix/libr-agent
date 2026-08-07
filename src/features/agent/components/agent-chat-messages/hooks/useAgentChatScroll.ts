import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { getLogger } from '@/lib/logger';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import type { Message } from '@/models/chat';
import type { useAgentChat } from '@/context/AgentChatContext';
import type { useAgentSession } from '@/context/AgentSessionContext';
import {
  INITIAL_FIRST_ITEM_INDEX,
  BOTTOM_FOLLOW_RELEASE_SCROLL_DISTANCE,
  NEAR_TOP_SCROLL_THRESHOLD,
} from '../types';
import {
  getInitialTopMostItemIndex,
  getVisualBottomThreshold,
  isTrustedVisualBottom,
  isBottomAlignmentActive,
  getScrollContentElement,
  isLayoutNeutralLatestMessageUpdate,
  isThinkingOnlyLatestMessageUpdate,
} from '../utils';

import { usePrependPreservation } from './subhooks/usePrependPreservation';
import { useBottomAlignment } from './subhooks/useBottomAlignment';
import { useScrollFollowState } from './subhooks/useScrollFollowState';
import { useScrollScheduler } from './subhooks/useScrollScheduler';
import { useHydrationTracking } from './subhooks/useHydrationTracking';

const logger = getLogger('AgentChatMessages');

export interface UseAgentChatScrollOptions {
  groupedMessages: GroupedMessage[];
  sessionId: string | undefined;
  latestMessage: Message | undefined;
  workflowStatus: ReturnType<typeof useAgentChat>['workflowStatus'];
  pendingApprovals: ReturnType<typeof useAgentSession>['pendingApprovals'];
  agentError: ReturnType<typeof useAgentChat>['error'];
  agentLlmError: ReturnType<typeof useAgentChat>['llmError'];
  /** True while an older-page fetch is in flight (header height may change). */
  isLoadingOlderMessages?: boolean;
}

export function useAgentChatScroll({
  groupedMessages,
  sessionId,
  latestMessage,
  workflowStatus,
  pendingApprovals,
  agentError,
  agentLlmError,
  isLoadingOlderMessages = false,
}: UseAgentChatScrollOptions) {
  const footerEndRef = useRef<HTMLDivElement | null>(null);
  const virtuosoRef = useRef<VirtuosoHandle | null>(null);
  const virtuosoAtBottomRef = useRef(false);
  const previousScrollTopRef = useRef<number | null>(null);
  const scrollTopRef = useRef(0);
  const groupedMessageCountRef = useRef(groupedMessages.length);
  const previousLatestMessageRef = useRef<Message | undefined>(latestMessage);

  const [scrollerElement, setScrollerElement] = useState<HTMLDivElement | null>(
    null,
  );
  const scrollerElementRef = useRef<HTMLDivElement | null>(null);
  scrollerElementRef.current = scrollerElement;

  const bottomThreshold = getVisualBottomThreshold();

  const scrollDebugStateRef = useRef({
    sessionId: sessionId,
    firstItemIndex: INITIAL_FIRST_ITEM_INDEX,
    effectiveFirstItemIndex: INITIAL_FIRST_ITEM_INDEX,
  });

  const initialTopMostItemIndex = useMemo(() => {
    return getInitialTopMostItemIndex(
      INITIAL_FIRST_ITEM_INDEX,
      groupedMessages.length,
    );
  }, [sessionId]);

  // Keep identity stable — logScrollState used to depend on scrollerElement and
  // cascaded into scheduleScrollToBottom / requestBottomAlignment, which
  // re-fired the session-reset effect (force bottom) on unrelated re-renders.
  const logScrollState = useCallback(
    (
      event: string,
      extra: Record<string, boolean | number | string | undefined> = {},
    ) => {
      if (!import.meta.env.DEV) {
        return;
      }

      const currentState = scrollDebugStateRef.current;
      logger.debug('[scroll-debug] AgentChatMessages', {
        event,
        sessionId: currentState.sessionId,
        groupedMessageCount: groupedMessageCountRef.current,
        firstItemIndex: currentState.firstItemIndex,
        effectiveFirstItemIndex: currentState.effectiveFirstItemIndex,
        isPinned: isPinnedToBottomRef.current,
        visualBottom: visualBottomRef.current,
        shouldFollowLatest: shouldFollowLatestRef.current,
        bottomAlignmentPhase: bottomAlignmentPhaseRef.current,
        hasVirtuosoHandle: !!virtuosoRef.current,
        hasScroller: !!scrollerElementRef.current,
        hasFooterSentinel: !!footerEndRef.current,
        ...extra,
      });
    },
    [],
  );

  // 1. Follow state management
  const {
    isPinned,
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
  } = useScrollFollowState({ logScrollState });

  // 2. Prepend preservation management
  const {
    firstItemIndex,
    effectiveFirstItemIndex,
    prependCount,
    isPreservingPrependPositionRef,
    prependStabilizeTimeoutRef,
  } = usePrependPreservation({
    groupedMessages,
    sessionId,
    logScrollState,
  });

  scrollDebugStateRef.current = {
    sessionId,
    firstItemIndex,
    effectiveFirstItemIndex,
  };

  groupedMessageCountRef.current = groupedMessages.length;

  // 3. Bottom alignment state machine
  const {
    bottomAlignmentPhaseRef,
    bottomAlignmentLayoutVersionRef,
    bottomAlignmentRequestedVersionRef,
    clearBottomAlignmentVerifyFrame,
    setBottomAlignmentPhase,
    requestBottomAlignment,
    abortBottomAlignment,
    markBottomAlignmentLayoutChanged,
    scheduleBottomAlignmentVerification,
  } = useBottomAlignment({
    isPreservingPrependPositionRef,
    logScrollState,
  });

  // 4. Scroll scheduler logic
  const { clearScheduledAutoScroll, scheduleScrollToBottom } =
    useScrollScheduler({
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
    });

  // 5. Hydration tracking
  const { hasHydratedMessagesRef } = useHydrationTracking({
    sessionId,
    groupedMessagesLength: groupedMessages.length,
    isPinnedToBottomRef,
    isPreservingPrependPositionRef,
    prependStabilizeTimeoutRef,
    requestBottomAlignment,
    scheduleScrollToBottom,
    logScrollState,
  });

  // Older-page fetch changes header height before messages arrive. Pause follow
  // immediately so ResizeObserver / height churn cannot yank to bottom.
  useEffect(() => {
    if (!isLoadingOlderMessages) {
      return;
    }

    isPreservingPrependPositionRef.current = true;
    // Treat as away-from-bottom so resumeBottomFollow('bottom-context-restored')
    // cannot re-arm follow while older messages are loading.
    visualBottomRef.current = false;
    enterHistoryBrowsing('loading-older-messages');
    abortBottomAlignment('loading-older-messages');
    clearScheduledAutoScroll();
    pauseBottomFollow('loading-older-messages');
    setEffectivePinnedState(false);
    logScrollState('loading-older:armed');
  }, [
    abortBottomAlignment,
    clearScheduledAutoScroll,
    enterHistoryBrowsing,
    isLoadingOlderMessages,
    isPreservingPrependPositionRef,
    logScrollState,
    pauseBottomFollow,
    setEffectivePinnedState,
    visualBottomRef,
  ]);

  const handleStartReached = useCallback(() => {
    // Virtuoso can fire startReached before scrollTop hits NEAR_TOP — pause
    // follow so the subsequent older-page load cannot auto-scroll to bottom.
    visualBottomRef.current = false;
    enterHistoryBrowsing('start-reached');
    abortBottomAlignment('start-reached');
    clearScheduledAutoScroll();
    pauseBottomFollow('start-reached');
    setEffectivePinnedState(false);
    logScrollState('start-reached:pause-follow');
  }, [
    abortBottomAlignment,
    clearScheduledAutoScroll,
    enterHistoryBrowsing,
    logScrollState,
    pauseBottomFollow,
    setEffectivePinnedState,
    visualBottomRef,
  ]);

  const handleResizeObserverLayoutChange = useCallback(
    (reason: 'content-resize-observer' | 'scroller-resize-observer') => {
      if (prependCount > 0 || isPreservingPrependPositionRef.current) {
        return;
      }

      // Mid upward-read: height changes must not force-follow the stream.
      if (upwardReleaseDistanceRef.current > 0 && !visualBottomRef.current) {
        markBottomAlignmentLayoutChanged(reason);
        return;
      }

      markBottomAlignmentLayoutChanged(reason);
      scheduleBottomAlignmentVerification(
        reason,
        visualBottomRef.current,
        virtuosoAtBottomRef.current,
      );
      if (
        !shouldFollowLatestRef.current &&
        !visualBottomRef.current &&
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current)
      ) {
        return;
      }

      scheduleScrollToBottom(reason, virtuosoAtBottomRef.current);
    },
    [
      isPreservingPrependPositionRef,
      markBottomAlignmentLayoutChanged,
      bottomAlignmentPhaseRef,
      prependCount,
      scheduleBottomAlignmentVerification,
      scheduleScrollToBottom,
      shouldFollowLatestRef,
      upwardReleaseDistanceRef,
      visualBottomRef,
    ],
  );

  const handleVirtuosoAtBottomStateChange = useCallback(
    (atBottom: boolean) => {
      virtuosoAtBottomRef.current = atBottom;
      logScrollState('virtuoso:atBottomStateChange', {
        atBottom,
      });
      if (atBottom) {
        scheduleBottomAlignmentVerification(
          'at-bottom-state-change',
          visualBottomRef.current,
          atBottom,
        );
      }
    },
    [logScrollState, scheduleBottomAlignmentVerification, visualBottomRef],
  );

  const handleTotalListHeightChanged = useCallback(
    (height: number) => {
      logScrollState('virtuoso:totalListHeightChanged', {
        height,
      });
      if (groupedMessageCountRef.current === 0) {
        logScrollState('virtuoso:totalListHeightChanged:skip', {
          height,
        });
        return;
      }

      if (prependCount > 0 || isPreservingPrependPositionRef.current) {
        logScrollState('virtuoso:totalListHeightChanged:prepend-skip', {
          height,
          prependCount,
        });
        return;
      }

      if (upwardReleaseDistanceRef.current > 0 && !visualBottomRef.current) {
        markBottomAlignmentLayoutChanged('total-list-height-changed');
        logScrollState('virtuoso:totalListHeightChanged:upward-skip', {
          height,
          upwardReleaseDistance: upwardReleaseDistanceRef.current,
        });
        return;
      }

      markBottomAlignmentLayoutChanged('total-list-height-changed');
      scheduleBottomAlignmentVerification(
        'total-list-height-changed',
        visualBottomRef.current,
        virtuosoAtBottomRef.current,
      );

      if (
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !shouldFollowLatestRef.current
      ) {
        logScrollState('virtuoso:totalListHeightChanged:skip', {
          height,
        });
        return;
      }

      scheduleScrollToBottom(
        'total-list-height-changed',
        virtuosoAtBottomRef.current,
      );
    },
    [
      bottomAlignmentPhaseRef,
      isPreservingPrependPositionRef,
      logScrollState,
      markBottomAlignmentLayoutChanged,
      prependCount,
      scheduleBottomAlignmentVerification,
      scheduleScrollToBottom,
      shouldFollowLatestRef,
      upwardReleaseDistanceRef,
      visualBottomRef,
    ],
  );

  // Session change only. Callbacks live in a ref so scroller attach / rerenders
  // cannot re-trigger force bottom (`session-changed` bypasses all suppressors).
  const sessionResetActionsRef = useRef({
    setBottomAlignmentPhase,
    clearBottomAlignmentVerifyFrame,
    setEffectivePinnedState,
    clearScheduledAutoScroll,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    exitHistoryBrowsing,
  });
  sessionResetActionsRef.current = {
    setBottomAlignmentPhase,
    clearBottomAlignmentVerifyFrame,
    setEffectivePinnedState,
    clearScheduledAutoScroll,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    exitHistoryBrowsing,
  };

  useEffect(() => {
    const {
      setBottomAlignmentPhase: setPhase,
      clearBottomAlignmentVerifyFrame: clearVerify,
      setEffectivePinnedState: setPinned,
      clearScheduledAutoScroll: clearAutoScroll,
      logScrollState: logState,
      requestBottomAlignment: requestAlignment,
      scheduleScrollToBottom: scheduleBottom,
      exitHistoryBrowsing: exitHistory,
    } = sessionResetActionsRef.current;

    setPhase('idle', 'session-reset');
    clearVerify();
    bottomAlignmentRequestedVersionRef.current =
      bottomAlignmentLayoutVersionRef.current;
    virtuosoAtBottomRef.current = false;
    visualBottomRef.current = true;
    shouldFollowLatestRef.current = true;
    isPreservingPrependPositionRef.current = false;
    upwardReleaseDistanceRef.current = 0;
    exitHistory('session-changed');
    setPinned(true);
    clearAutoScroll();
    if (prependStabilizeTimeoutRef.current !== null) {
      window.clearTimeout(prependStabilizeTimeoutRef.current);
      prependStabilizeTimeoutRef.current = null;
    }
    previousScrollTopRef.current = null;
    scrollTopRef.current = 0;
    logState('sessionEffect:reset-bottom-alignment');
    requestAlignment('session-changed');
    scheduleBottom('session-changed', false);
  }, [sessionId]);

  // Unmount cleanup
  useEffect(() => {
    return () => {
      clearScheduledAutoScroll();
      if (prependStabilizeTimeoutRef.current !== null) {
        window.clearTimeout(prependStabilizeTimeoutRef.current);
        prependStabilizeTimeoutRef.current = null;
      }
      clearBottomAlignmentVerifyFrame();
    };
  }, [
    clearBottomAlignmentVerifyFrame,
    clearScheduledAutoScroll,
    prependStabilizeTimeoutRef,
  ]);

  // DOM scroll listener
  useEffect(() => {
    if (!scrollerElement) {
      return;
    }

    const updatePinnedState = () => {
      const currentScrollTop = scrollerElement.scrollTop;
      const previousScrollTop = previousScrollTopRef.current;
      const scrollDelta =
        previousScrollTop === null ? 0 : currentScrollTop - previousScrollTop;
      previousScrollTopRef.current = currentScrollTop;
      scrollTopRef.current = currentScrollTop;
      const distanceFromBottom =
        scrollerElement.scrollHeight -
        currentScrollTop -
        scrollerElement.clientHeight;
      const isSelfScroll = performance.now() < selfScrollIgnoreUntilRef.current;

      // Prepend/older-page layout can briefly report distanceFromBottom≈0 while
      // scrollTop is still at the top. Never treat that as "reached bottom".
      const trustedVisualBottom = isTrustedVisualBottom(
        distanceFromBottom,
        currentScrollTop,
        {
          threshold: bottomThreshold,
          scrollHeight: scrollerElement.scrollHeight,
          clientHeight: scrollerElement.clientHeight,
        },
      );
      visualBottomRef.current = trustedVisualBottom;

      if (trustedVisualBottom) {
        upwardReleaseDistanceRef.current = 0;
        // Leave history browsing only when the user is actually at the latest
        // end — not when top-edge / collapsed-height falsely reports bottom.
        if (isHistoryBrowsingRef.current) {
          if (currentScrollTop > NEAR_TOP_SCROLL_THRESHOLD) {
            exitHistoryBrowsing('bottom-reached');
            resumeBottomFollow('bottom-reached');
          }
        } else {
          resumeBottomFollow('bottom-reached');
        }
      } else {
        if (scrollDelta > 0) {
          upwardReleaseDistanceRef.current = 0;
        } else if (
          !isPreservingPrependPositionRef.current &&
          scrollDelta < 0 &&
          !isSelfScroll
        ) {
          upwardReleaseDistanceRef.current += Math.abs(scrollDelta);
          if (
            shouldFollowLatestRef.current &&
            upwardReleaseDistanceRef.current >=
              BOTTOM_FOLLOW_RELEASE_SCROLL_DISTANCE
          ) {
            abortBottomAlignment('explicit-scroll-up');
            clearScheduledAutoScroll();
            pauseBottomFollow('explicit-scroll-up');
          }
        }

        if (
          shouldFollowLatestRef.current &&
          !isPreservingPrependPositionRef.current &&
          !isSelfScroll &&
          currentScrollTop <= NEAR_TOP_SCROLL_THRESHOLD
        ) {
          enterHistoryBrowsing('reached-top');
          abortBottomAlignment('reached-top');
          clearScheduledAutoScroll();
          pauseBottomFollow('reached-top');
        }
      }

      if (!trustedVisualBottom && !shouldFollowLatestRef.current) {
        virtuosoAtBottomRef.current = false;
        abortBottomAlignment('visual-bottom-lost');
        clearScheduledAutoScroll();
      }

      scheduleBottomAlignmentVerification(
        'scroll:updatePinnedState',
        trustedVisualBottom,
        virtuosoAtBottomRef.current,
      );

      setEffectivePinnedState(
        trustedVisualBottom || shouldFollowLatestRef.current,
      );
      logScrollState('scroll:updatePinnedState', {
        currentScrollTop,
        scrollDelta,
        distanceFromBottom,
        trustedVisualBottom,
        isSelfScroll,
        upwardReleaseDistance: upwardReleaseDistanceRef.current,
      });
    };

    const handleScroll = () => {
      updatePinnedState();
    };

    scrollerElement.addEventListener('scroll', handleScroll, { passive: true });
    updatePinnedState();

    return () => {
      scrollerElement.removeEventListener('scroll', handleScroll);
    };
  }, [
    abortBottomAlignment,
    bottomThreshold,
    clearScheduledAutoScroll,
    enterHistoryBrowsing,
    exitHistoryBrowsing,
    isHistoryBrowsingRef,
    isPreservingPrependPositionRef,
    logScrollState,
    pauseBottomFollow,
    resumeBottomFollow,
    scheduleBottomAlignmentVerification,
    scrollerElement,
    selfScrollIgnoreUntilRef,
    setEffectivePinnedState,
    shouldFollowLatestRef,
    upwardReleaseDistanceRef,
    visualBottomRef,
  ]);

  const handleManualScrollToBottom = useCallback(() => {
    visualBottomRef.current = true;
    upwardReleaseDistanceRef.current = 0;
    exitHistoryBrowsing('manual-scroll-to-bottom');
    requestBottomAlignment('manual-scroll-to-bottom');
    resumeBottomFollow('manual-scroll-to-bottom');
    setEffectivePinnedState(true);
    logScrollState('scrollToBottom:manual');
    scheduleScrollToBottom(
      'manual-scroll-to-bottom',
      virtuosoAtBottomRef.current,
    );
  }, [
    exitHistoryBrowsing,
    logScrollState,
    requestBottomAlignment,
    resumeBottomFollow,
    scheduleScrollToBottom,
    setEffectivePinnedState,
    upwardReleaseDistanceRef,
    visualBottomRef,
  ]);

  // Content ResizeObserver
  useEffect(() => {
    const content = getScrollContentElement(scrollerElement);

    if (!content || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      handleResizeObserverLayoutChange('content-resize-observer');
    });

    observer.observe(content);

    return () => {
      observer.disconnect();
    };
  }, [handleResizeObserverLayoutChange, scrollerElement]);

  // Scroller ResizeObserver
  useEffect(() => {
    if (!scrollerElement || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      handleResizeObserverLayoutChange('scroller-resize-observer');
    });

    observer.observe(scrollerElement);

    return () => {
      observer.disconnect();
    };
  }, [handleResizeObserverLayoutChange, scrollerElement]);

  // Reactive state change tracking
  useEffect(() => {
    const previousLatestMessage = previousLatestMessageRef.current;
    previousLatestMessageRef.current = latestMessage;

    if (
      groupedMessages.length > 0 &&
      hasHydratedMessagesRef.current.hasMessages &&
      visualBottomRef.current &&
      scrollTopRef.current > NEAR_TOP_SCROLL_THRESHOLD &&
      !isPreservingPrependPositionRef.current &&
      !isLoadingOlderMessages
    ) {
      if (isHistoryBrowsingRef.current) {
        exitHistoryBrowsing('bottom-context-restored');
      }
      resumeBottomFollow('bottom-context-restored');
    }

    if (
      prependCount > 0 ||
      isPreservingPrependPositionRef.current ||
      isLoadingOlderMessages ||
      isHistoryBrowsingRef.current
    ) {
      logScrollState('reactive-state-change:prepend-skip', {
        prependCount,
        messageId: latestMessage?.id,
      });
      return;
    }

    if (!shouldFollowLatestRef.current) {
      return;
    }

    if (
      isLayoutNeutralLatestMessageUpdate(previousLatestMessage, latestMessage)
    ) {
      logScrollState(
        isThinkingOnlyLatestMessageUpdate(previousLatestMessage, latestMessage)
          ? 'reactive-state-change:thinking-only-skip'
          : 'reactive-state-change:layout-neutral-skip',
        {
          messageId: latestMessage?.id,
        },
      );
      return;
    }

    scheduleScrollToBottom(
      'reactive-state-change',
      virtuosoAtBottomRef.current,
    );
  }, [
    latestMessage,
    workflowStatus,
    pendingApprovals?.length,
    agentError,
    agentLlmError,
    groupedMessages.length,
    hasHydratedMessagesRef,
    isHistoryBrowsingRef,
    isLoadingOlderMessages,
    isPreservingPrependPositionRef,
    logScrollState,
    prependCount,
    exitHistoryBrowsing,
    resumeBottomFollow,
    scheduleScrollToBottom,
    shouldFollowLatestRef,
    scrollTopRef,
    visualBottomRef,
  ]);

  return {
    virtuosoRef,
    footerEndRef,
    scrollerElement,
    setScrollerElement,
    effectiveFirstItemIndex,
    isPinned,
    handleVirtuosoAtBottomStateChange,
    handleTotalListHeightChanged,
    handleManualScrollToBottom,
    handleStartReached,
    initialTopMostItemIndex,
    bottomThreshold,
    logScrollState,
  };
}
