import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { type VirtuosoHandle } from 'react-virtuoso';
import { getLogger } from '@/lib/logger';
import { type GroupedMessage } from '@/hooks/useMessageGrouping';
import { type Message } from '@/models/chat';
import { type useAgentChat } from '@/context/AgentChatContext';
import { type useAgentSession } from '@/context/AgentSessionContext';
import {
  INITIAL_FIRST_ITEM_INDEX,
  BOTTOM_FOLLOW_RELEASE_SCROLL_DISTANCE,
  SELF_SCROLL_IGNORE_WINDOW_MS,
  type BottomAlignmentPhase,
} from '../types';
import {
  getPrependedFirstItemIndex,
  getInitialTopMostItemIndex,
  getVisualBottomThreshold,
  isPinnedToBottom,
  isBottomAlignmentActive,
  scrollFooterSentinelIntoView,
  scrollVirtuosoToBottom,
  getScrollContentElement,
  findGroupedMessageIndexByBoundary,
} from '../utils';

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
  const visualBottomRef = useRef(true);
  const isPinnedToBottomRef = useRef(true);
  const shouldFollowLatestRef = useRef(true);
  const bottomAlignmentPhaseRef = useRef<BottomAlignmentPhase>('idle');
  const bottomAlignmentLayoutVersionRef = useRef(0);
  const bottomAlignmentRequestedVersionRef = useRef(0);
  const bottomAlignmentVerifyFrameRef = useRef<number | null>(null);
  const prependStabilizeTimeoutRef = useRef<number | null>(null);
  const isPreservingPrependPositionRef = useRef(false);
  const autoScrollFrameRef = useRef<number | null>(null);
  const selfScrollIgnoreUntilRef = useRef(0);
  const previousScrollTopRef = useRef<number | null>(null);
  const upwardReleaseDistanceRef = useRef(0);
  const groupedMessageCountRef = useRef(groupedMessages.length);
  const hasHydratedMessagesRef = useRef<{
    sessionId: string | undefined;
    hasMessages: boolean;
  }>({
    sessionId: undefined,
    hasMessages: false,
  });

  const [firstItemIndex, setFirstItemIndex] = useState(
    INITIAL_FIRST_ITEM_INDEX,
  );
  const [isPinned, setIsPinned] = useState(true);
  const [scrollerElement, setScrollerElement] = useState<HTMLDivElement | null>(
    null,
  );

  const bottomThreshold = getVisualBottomThreshold();
  const forceBottomScrollReasons = useMemo(
    () =>
      new Set([
        'manual-scroll-to-bottom',
        'session-changed',
        'hydrated-messages-arrived',
      ]),
    [],
  );

  const previousListStateRef = useRef<{
    firstId: string | undefined;
    lastId: string | undefined;
    length: number;
    sessionId: string | undefined;
  }>({
    firstId: undefined,
    lastId: undefined,
    length: 0,
    sessionId: undefined,
  });

  const previousListState = previousListStateRef.current;
  const hasMessages = groupedMessages.length > 0;
  const currentFirstGroupedMessageId = hasMessages
    ? groupedMessages[0]?.message.id
    : undefined;
  const currentLastGroupedMessageId = hasMessages
    ? groupedMessages[groupedMessages.length - 1]?.message.id
    : undefined;
  const didSessionChangeForListState =
    previousListState.sessionId !== sessionId;

  const previousHeadIndexInCurrentList =
    !didSessionChangeForListState &&
    groupedMessages.length > previousListState.length &&
    previousListState.lastId === currentLastGroupedMessageId
      ? findGroupedMessageIndexByBoundary(
          groupedMessages,
          previousListState.firstId,
        )
      : -1;

  const candidatePrependCount =
    previousHeadIndexInCurrentList > 0 ? previousHeadIndexInCurrentList : 0;
  const preservesPreviousVisibleHead = previousHeadIndexInCurrentList >= 0;
  const prependCount = candidatePrependCount;

  const effectiveFirstItemIndex = didSessionChangeForListState
    ? INITIAL_FIRST_ITEM_INDEX
    : prependCount > 0
      ? getPrependedFirstItemIndex(firstItemIndex, prependCount)
      : firstItemIndex;

  const scrollDebugStateRef = useRef({
    sessionId: sessionId,
    firstItemIndex,
    effectiveFirstItemIndex,
  });

  const initialTopMostItemIndex = useMemo(() => {
    return getInitialTopMostItemIndex(
      INITIAL_FIRST_ITEM_INDEX,
      groupedMessages.length,
    );
    // Calculate initial position once when the session changes.
  }, [sessionId]);

  scrollDebugStateRef.current = {
    sessionId: sessionId,
    firstItemIndex,
    effectiveFirstItemIndex,
  };

  if (
    import.meta.env.DEV &&
    groupedMessages.length > previousListState.length &&
    previousListState.lastId === currentLastGroupedMessageId &&
    previousListState.firstId &&
    !preservesPreviousVisibleHead
  ) {
    logger.warn('[scroll-debug] prepend-invariant-violated', {
      sessionId: sessionId,
      previousFirstId: previousListState.firstId,
      previousLastId: previousListState.lastId,
      currentFirstId: currentFirstGroupedMessageId,
      currentLastId: currentLastGroupedMessageId,
      candidatePrependCount,
      currentCandidateAnchorId:
        candidatePrependCount > 0
          ? groupedMessages[candidatePrependCount]?.message.id
          : undefined,
      previousHeadIndexInCurrentList,
      previousLength: previousListState.length,
      currentLength: groupedMessages.length,
    });
  }

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

  groupedMessageCountRef.current = groupedMessages.length;

  const setEffectivePinnedState = useCallback((nextPinned: boolean) => {
    isPinnedToBottomRef.current = nextPinned;
    setIsPinned(nextPinned);
  }, []);

  const clearBottomAlignmentVerifyFrame = useCallback(() => {
    if (bottomAlignmentVerifyFrameRef.current !== null) {
      cancelAnimationFrame(bottomAlignmentVerifyFrameRef.current);
      bottomAlignmentVerifyFrameRef.current = null;
    }
  }, []);

  const clearScheduledAutoScroll = useCallback(() => {
    if (autoScrollFrameRef.current !== null) {
      cancelAnimationFrame(autoScrollFrameRef.current);
      autoScrollFrameRef.current = null;
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
    [logScrollState],
  );

  const maybeCompleteBottomAlignment = useCallback(
    (reason: string) => {
      if (bottomAlignmentPhaseRef.current !== 'verifying') {
        return;
      }

      if (isPreservingPrependPositionRef.current) {
        return;
      }

      if (!visualBottomRef.current || !virtuosoAtBottomRef.current) {
        logScrollState('bottom-alignment:pending', {
          reason,
          visualBottom: visualBottomRef.current,
          virtuosoAtBottom: virtuosoAtBottomRef.current,
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
          maybeCompleteBottomAlignment('version-invalidated');
        });
        return;
      }

      clearBottomAlignmentVerifyFrame();
      setBottomAlignmentPhase('aligned', reason);
    },
    [clearBottomAlignmentVerifyFrame, logScrollState, setBottomAlignmentPhase],
  );

  const scheduleBottomAlignmentVerification = useCallback(
    (reason: string) => {
      if (!isBottomAlignmentActive(bottomAlignmentPhaseRef.current)) {
        return;
      }

      clearBottomAlignmentVerifyFrame();
      bottomAlignmentVerifyFrameRef.current = requestAnimationFrame(() => {
        bottomAlignmentVerifyFrameRef.current = null;
        maybeCompleteBottomAlignment(reason);
      });
    },
    [clearBottomAlignmentVerifyFrame, maybeCompleteBottomAlignment],
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

  const executeScrollToBottom = useCallback(
    (reason: string): boolean => {
      const itemCount = groupedMessageCountRef.current;
      selfScrollIgnoreUntilRef.current =
        performance.now() + SELF_SCROLL_IGNORE_WINDOW_MS;
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
        return true;
      }

      if (footerEndRef.current) {
        logScrollState('executeScrollToBottom:footer-sentinel-fallback', {
          itemCount,
          reason,
        });
        scrollFooterSentinelIntoView(footerEndRef.current);
        return true;
      }

      logScrollState('executeScrollToBottom:unavailable', {
        itemCount,
        reason,
        scrolledWithVirtuoso: false,
      });
      return false;
    },
    [logScrollState],
  );

  useEffect(() => {
    if (didSessionChangeForListState) {
      logScrollState('listState:session-changed', {
        previousSessionId: previousListState.sessionId,
        nextSessionId: sessionId,
        groupedMessageCount: groupedMessages.length,
      });
      setFirstItemIndex(INITIAL_FIRST_ITEM_INDEX);
    } else if (prependCount > 0) {
      isPreservingPrependPositionRef.current = true;
      if (prependStabilizeTimeoutRef.current !== null) {
        window.clearTimeout(prependStabilizeTimeoutRef.current);
      }
      prependStabilizeTimeoutRef.current = window.setTimeout(() => {
        prependStabilizeTimeoutRef.current = null;
        isPreservingPrependPositionRef.current = false;
        logScrollState('prepend-preservation:settled', {
          prependCount,
        });
      }, 250);
      logScrollState('prepend-preservation:start', {
        prependCount,
      });
      setFirstItemIndex(effectiveFirstItemIndex);
    }

    previousListStateRef.current = {
      firstId: currentFirstGroupedMessageId,
      lastId: currentLastGroupedMessageId,
      length: groupedMessages.length,
      sessionId: sessionId,
    };
  }, [
    currentFirstGroupedMessageId,
    currentLastGroupedMessageId,
    didSessionChangeForListState,
    effectiveFirstItemIndex,
    groupedMessages.length,
    logScrollState,
    prependCount,
    sessionId,
  ]);

  const scheduleScrollToBottom = useCallback(
    (reason: string) => {
      const shouldForce =
        forceBottomScrollReasons.has(reason) || shouldFollowLatestRef.current;
      const shouldSuppressForPrepend =
        isPreservingPrependPositionRef.current &&
        !forceBottomScrollReasons.has(reason);

      const shouldSuppressForUserScroll =
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !visualBottomRef.current &&
        !shouldForce;

      if (shouldSuppressForPrepend || shouldSuppressForUserScroll) {
        clearScheduledAutoScroll();
        logScrollState('scheduleScrollToBottom:suppressed', {
          reason,
          shouldSuppressForPrepend,
          shouldSuppressForUserScroll,
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
        logScrollState('scheduleScrollToBottom:frame-fired', {
          reason,
        });
        const didScroll = executeScrollToBottom(reason);
        if (didScroll && bottomAlignmentPhaseRef.current === 'requesting') {
          bottomAlignmentRequestedVersionRef.current =
            bottomAlignmentLayoutVersionRef.current;
          setBottomAlignmentPhase('verifying', `scroll-dispatched:${reason}`);
          scheduleBottomAlignmentVerification(`scroll-dispatched:${reason}`);
        }
      });
    },
    [
      clearScheduledAutoScroll,
      executeScrollToBottom,
      forceBottomScrollReasons,
      logScrollState,
      scheduleBottomAlignmentVerification,
      setBottomAlignmentPhase,
    ],
  );

  const handleVirtuosoAtBottomStateChange = useCallback(
    (atBottom: boolean) => {
      virtuosoAtBottomRef.current = atBottom;
      logScrollState('virtuoso:atBottomStateChange', {
        atBottom,
      });
      if (atBottom) {
        scheduleBottomAlignmentVerification('at-bottom-state-change');
      }
    },
    [logScrollState, scheduleBottomAlignmentVerification],
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
      scheduleBottomAlignmentVerification('total-list-height-changed');

      if (
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !isPinnedToBottomRef.current
      ) {
        logScrollState('virtuoso:totalListHeightChanged:skip', {
          height,
        });
        return;
      }

      scheduleScrollToBottom('total-list-height-changed');
    },
    [
      logScrollState,
      markBottomAlignmentLayoutChanged,
      prependCount,
      scheduleBottomAlignmentVerification,
      scheduleScrollToBottom,
    ],
  );

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
    logScrollState('sessionEffect:reset-bottom-alignment');
    requestBottomAlignment('session-changed');
    scheduleScrollToBottom('session-changed');
  }, [
    clearBottomAlignmentVerifyFrame,
    clearScheduledAutoScroll,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    sessionId,
    setEffectivePinnedState,
    setBottomAlignmentPhase,
  ]);

  useEffect(() => {
    return () => {
      clearScheduledAutoScroll();
      if (prependStabilizeTimeoutRef.current !== null) {
        window.clearTimeout(prependStabilizeTimeoutRef.current);
        prependStabilizeTimeoutRef.current = null;
      }
      clearBottomAlignmentVerifyFrame();
    };
  }, [clearBottomAlignmentVerifyFrame, clearScheduledAutoScroll]);

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
      }

      if (!visualPinned && !shouldFollowLatestRef.current) {
        virtuosoAtBottomRef.current = false;
        abortBottomAlignment('visual-bottom-lost');
        clearScheduledAutoScroll();
      }

      scheduleBottomAlignmentVerification('scroll:updatePinnedState');

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
    logScrollState,
    pauseBottomFollow,
    resumeBottomFollow,
    scheduleBottomAlignmentVerification,
    scrollerElement,
    setEffectivePinnedState,
  ]);

  useEffect(() => {
    const trackedSessionId = hasHydratedMessagesRef.current.sessionId;

    if (trackedSessionId !== sessionId) {
      hasHydratedMessagesRef.current = {
        sessionId: sessionId,
        hasMessages: groupedMessages.length > 0,
      };
      logScrollState('hydration:tracked-session-changed', {
        hasMessages: groupedMessages.length > 0,
      });
      return;
    }

    if (
      !hasHydratedMessagesRef.current.hasMessages &&
      groupedMessages.length > 0 &&
      isPinnedToBottomRef.current
    ) {
      isPreservingPrependPositionRef.current = false;
      if (prependStabilizeTimeoutRef.current !== null) {
        window.clearTimeout(prependStabilizeTimeoutRef.current);
        prependStabilizeTimeoutRef.current = null;
      }
      requestBottomAlignment('hydrated-messages-arrived');
      logScrollState('hydration:messages-arrived');
      scheduleScrollToBottom('hydrated-messages-arrived');
    }

    if (groupedMessages.length > 0) {
      hasHydratedMessagesRef.current.hasMessages = true;
    }
  }, [
    groupedMessages.length,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    sessionId,
  ]);

  const handleManualScrollToBottom = useCallback(() => {
    visualBottomRef.current = true;
    upwardReleaseDistanceRef.current = 0;
    requestBottomAlignment('manual-scroll-to-bottom');
    resumeBottomFollow('manual-scroll-to-bottom');
    setEffectivePinnedState(true);
    logScrollState('scrollToBottom:manual');
    scheduleScrollToBottom('manual-scroll-to-bottom');
  }, [
    logScrollState,
    requestBottomAlignment,
    resumeBottomFollow,
    scheduleScrollToBottom,
    setEffectivePinnedState,
  ]);

  useEffect(() => {
    const content = getScrollContentElement(scrollerElement);

    if (!content || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (prependCount > 0 || isPreservingPrependPositionRef.current) {
        return;
      }

      markBottomAlignmentLayoutChanged('content-resize-observer');
      scheduleBottomAlignmentVerification('content-resize-observer');
      if (
        !shouldFollowLatestRef.current &&
        !visualBottomRef.current &&
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current)
      ) {
        return;
      }

      scheduleScrollToBottom('content-resize-observer');
    });

    observer.observe(content);

    return () => {
      observer.disconnect();
    };
  }, [
    markBottomAlignmentLayoutChanged,
    prependCount,
    scheduleBottomAlignmentVerification,
    scheduleScrollToBottom,
    scrollerElement,
  ]);

  useEffect(() => {
    if (!scrollerElement || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (prependCount > 0 || isPreservingPrependPositionRef.current) {
        return;
      }

      markBottomAlignmentLayoutChanged('scroller-resize-observer');
      scheduleBottomAlignmentVerification('scroller-resize-observer');
      if (
        !shouldFollowLatestRef.current &&
        !visualBottomRef.current &&
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current)
      ) {
        return;
      }

      scheduleScrollToBottom('scroller-resize-observer');
    });

    observer.observe(scrollerElement);

    return () => {
      observer.disconnect();
    };
  }, [
    markBottomAlignmentLayoutChanged,
    prependCount,
    scheduleBottomAlignmentVerification,
    scheduleScrollToBottom,
    scrollerElement,
  ]);

  useEffect(() => {
    if (
      groupedMessages.length > 0 &&
      hasHydratedMessagesRef.current.hasMessages &&
      (visualBottomRef.current ||
        isBottomAlignmentActive(bottomAlignmentPhaseRef.current))
    ) {
      resumeBottomFollow('bottom-context-restored');
    }

    if (!shouldFollowLatestRef.current && !isPinnedToBottomRef.current) {
      return;
    }

    scheduleScrollToBottom('reactive-state-change');
  }, [
    latestMessage,
    workflowStatus,
    pendingApprovals?.length,
    agentError,
    agentLlmError,
    groupedMessages.length,
    resumeBottomFollow,
    scheduleScrollToBottom,
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
