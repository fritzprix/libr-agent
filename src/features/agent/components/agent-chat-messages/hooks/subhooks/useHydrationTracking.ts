import { useEffect, useRef, type MutableRefObject } from 'react';

export interface UseHydrationTrackingOptions {
  sessionId: string | undefined;
  groupedMessagesLength: number;
  isPinnedToBottomRef: MutableRefObject<boolean>;
  isPreservingPrependPositionRef: MutableRefObject<boolean>;
  prependStabilizeTimeoutRef: MutableRefObject<number | null>;
  requestBottomAlignment: (reason: string) => void;
  scheduleScrollToBottom: (reason: string) => void;
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
}

export function useHydrationTracking({
  sessionId,
  groupedMessagesLength,
  isPinnedToBottomRef,
  isPreservingPrependPositionRef,
  prependStabilizeTimeoutRef,
  requestBottomAlignment,
  scheduleScrollToBottom,
  logScrollState,
}: UseHydrationTrackingOptions) {
  const hasHydratedMessagesRef = useRef<{
    sessionId: string | undefined;
    hasMessages: boolean;
  }>({
    sessionId: undefined,
    hasMessages: false,
  });

  useEffect(() => {
    const trackedSessionId = hasHydratedMessagesRef.current.sessionId;

    if (trackedSessionId !== sessionId) {
      hasHydratedMessagesRef.current = {
        sessionId: sessionId,
        hasMessages: groupedMessagesLength > 0,
      };
      logScrollState('hydration:tracked-session-changed', {
        hasMessages: groupedMessagesLength > 0,
      });
      return;
    }

    if (
      !hasHydratedMessagesRef.current.hasMessages &&
      groupedMessagesLength > 0 &&
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

    if (groupedMessagesLength > 0) {
      hasHydratedMessagesRef.current.hasMessages = true;
    }
  }, [
    groupedMessagesLength,
    isPinnedToBottomRef,
    isPreservingPrependPositionRef,
    logScrollState,
    prependStabilizeTimeoutRef,
    requestBottomAlignment,
    scheduleScrollToBottom,
    sessionId,
  ]);

  return {
    hasHydratedMessagesRef,
  };
}
