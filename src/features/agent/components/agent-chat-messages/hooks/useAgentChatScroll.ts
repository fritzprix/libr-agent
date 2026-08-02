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
  isPinnedToBottom,
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
}

export function useAgentChatScroll({
  groupedMessages,
  sessionId,
  latestMessage,
  workflowStatus,
  pendingApprovals,
  agentError,
  agentLlmError,
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
        hasScroller: !!scrollerElement,
        hasFooterSentinel: !!footerEndRef.current,
        ...extra,
      });
    },
    [scrollerElement],
  );

  // 1. Follow state management
  const {
    isPinned,
    isPinnedToBottomRef,
    visualBottomRef,
    shouldFollowLatestRef,
    upwardReleaseDistanceRef,
    selfScrollIgnoreUntilRef,
    setEffectivePinnedState,
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
    virtuosoRef,
    selfScrollIgnoreUntilRef,
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

  const handleResizeObserverLayoutChange = useCallback(
    (reason: 'content-resize-observer' | 'scroller-resize-observer') => {
      if (prependCount > 0 || isPreservingPrependPositionRef.current) {
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

      markBottomAlignmentLayoutChanged('total-list-height-changed');
      scheduleBottomAlignmentVerification(
        'total-list-height-changed',
        visualBottomRef.current,
        virtuosoAtBottomRef.current,
      );

      if (
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !isPinnedToBottomRef.current
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
      isPinnedToBottomRef,
      isPreservingPrependPositionRef,
      logScrollState,
      markBottomAlignmentLayoutChanged,
      prependCount,
      scheduleBottomAlignmentVerification,
      scheduleScrollToBottom,
      visualBottomRef,
    ],
  );

  // Session change reset effect
  useEffect(() => {
    setBottomAlignmentPhase('idle', 'session-reset');
    clearBottomAlignmentVerifyFrame();
    bottomAlignmentRequestedVersionRef.current =
      bottomAlignmentLayoutVersionRef.current;
    virtuosoAtBottomRef.current = false;
    visualBottomRef.current = true;
    shouldFollowLatestRef.current = true;
    isPreservingPrependPositionRef.current = false;
    upwardReleaseDistanceRef.current = 0;
    setEffectivePinnedState(true);
    clearScheduledAutoScroll();
    if (prependStabilizeTimeoutRef.current !== null) {
      window.clearTimeout(prependStabilizeTimeoutRef.current);
      prependStabilizeTimeoutRef.current = null;
    }
    previousScrollTopRef.current = null;
    scrollTopRef.current = 0;
    logScrollState('sessionEffect:reset-bottom-alignment');
    requestBottomAlignment('session-changed');
    scheduleScrollToBottom('session-changed', false);
  }, [
    bottomAlignmentLayoutVersionRef,
    bottomAlignmentRequestedVersionRef,
    clearBottomAlignmentVerifyFrame,
    clearScheduledAutoScroll,
    isPreservingPrependPositionRef,
    logScrollState,
    prependStabilizeTimeoutRef,
    requestBottomAlignment,
    scheduleScrollToBottom,
    sessionId,
    setBottomAlignmentPhase,
    setEffectivePinnedState,
    shouldFollowLatestRef,
    upwardReleaseDistanceRef,
    visualBottomRef,
  ]);

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
      const visualPinned = isPinnedToBottom(
        distanceFromBottom,
        bottomThreshold,
      );
      const isSelfScroll = performance.now() < selfScrollIgnoreUntilRef.current;

      visualBottomRef.current = visualPinned;

      if (visualPinned) {
        upwardReleaseDistanceRef.current = 0;
        resumeBottomFollow('bottom-reached');
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
          abortBottomAlignment('reached-top');
          clearScheduledAutoScroll();
          pauseBottomFollow('reached-top');
        }
      }

      if (!visualPinned && !shouldFollowLatestRef.current) {
        virtuosoAtBottomRef.current = false;
        abortBottomAlignment('visual-bottom-lost');
        clearScheduledAutoScroll();
      }

      scheduleBottomAlignmentVerification(
        'scroll:updatePinnedState',
        visualPinned,
        virtuosoAtBottomRef.current,
      );

      setEffectivePinnedState(visualPinned || shouldFollowLatestRef.current);
      logScrollState('scroll:updatePinnedState', {
        currentScrollTop,
        scrollDelta,
        distanceFromBottom,
        visualPinned,
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
    requestBottomAlignment('manual-scroll-to-bottom');
    resumeBottomFollow('manual-scroll-to-bottom');
    setEffectivePinnedState(true);
    logScrollState('scrollToBottom:manual');
    scheduleScrollToBottom(
      'manual-scroll-to-bottom',
      virtuosoAtBottomRef.current,
    );
  }, [
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
      visualBottomRef.current
    ) {
      resumeBottomFollow('bottom-context-restored');
    }

    if (prependCount > 0 || isPreservingPrependPositionRef.current) {
      logScrollState('reactive-state-change:prepend-skip', {
        prependCount,
        messageId: latestMessage?.id,
      });
      return;
    }

    if (!shouldFollowLatestRef.current && !isPinnedToBottomRef.current) {
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
    isPinnedToBottomRef,
    isPreservingPrependPositionRef,
    logScrollState,
    prependCount,
    resumeBottomFollow,
    scheduleScrollToBottom,
    shouldFollowLatestRef,
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
    initialTopMostItemIndex,
    bottomThreshold,
    logScrollState,
  };
}
