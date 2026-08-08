import { useEffect, useRef, useState } from 'react';
import type { GroupedMessage } from '@/hooks/useMessageGrouping';
import { getLogger } from '@/lib/logger';
import { INITIAL_FIRST_ITEM_INDEX } from '../../types';
import {
  findGroupedMessageIndexByBoundary,
  getPrependedFirstItemIndex,
} from '../../utils';

const logger = getLogger('AgentChatMessages');

export interface PreviousListState {
  firstId: string | undefined;
  lastId: string | undefined;
  length: number;
  sessionId: string | undefined;
}

export interface UsePrependPreservationOptions {
  groupedMessages: GroupedMessage[];
  sessionId: string | undefined;
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
}

export function usePrependPreservation({
  groupedMessages,
  sessionId,
  logScrollState,
}: UsePrependPreservationOptions) {
  const isPreservingPrependPositionRef = useRef(false);
  const prependStabilizeTimeoutRef = useRef<number | null>(null);

  const [firstItemIndex, setFirstItemIndex] = useState(
    INITIAL_FIRST_ITEM_INDEX,
  );

  const previousListStateRef = useRef<PreviousListState>({
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

  // Arm prepend preservation during render so same-frame ResizeObserver / reactive scroll
  // cannot yank to bottom before the effect runs.
  if (prependCount > 0) {
    isPreservingPrependPositionRef.current = true;
  }

  // Virtuoso preserves scroll when firstItemIndex decreases by prependCount.
  // Do not also scrollToIndex(align:'start') — that double-corrects and nudges
  // the viewport (especially mid-list / startReached overscan loads).
  const effectiveFirstItemIndex = didSessionChangeForListState
    ? INITIAL_FIRST_ITEM_INDEX
    : prependCount > 0
      ? getPrependedFirstItemIndex(firstItemIndex, prependCount)
      : firstItemIndex;

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
        effectiveFirstItemIndex,
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

  return {
    firstItemIndex,
    setFirstItemIndex,
    effectiveFirstItemIndex,
    prependCount,
    didSessionChangeForListState,
    isPreservingPrependPositionRef,
    prependStabilizeTimeoutRef,
    previousListStateRef,
  };
}
