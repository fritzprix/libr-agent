import {
  type ComponentPropsWithoutRef,
  type ForwardedRef,
  type MutableRefObject,
  forwardRef,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
import { useLLMService } from '@/context/LLMServiceContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useFileRefetcher } from '@/features/agent/hooks/useFileRefetcher';
import {
  useMessageGrouping,
  type GroupedMessage,
} from '@/hooks/useMessageGrouping';
import { AgentMessageBubble } from './AgentMessageBubble';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from './shared';
import { CompactEventDivider } from './shared/CompactEventDivider';
import { Bot, ChevronDown } from 'lucide-react';
import { PendingApprovalWidget } from './PendingApprovalWidget';
import { getLogger } from '@/lib/logger';
import type { Message } from '@/models/chat';
import { useTranslation } from 'react-i18next';
import {
  Virtuoso,
  type Components,
  type IndexLocationWithAlign,
  type ListProps,
  type VirtuosoHandle,
} from 'react-virtuoso';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';

const logger = getLogger('AgentChatMessages');
const INITIAL_FIRST_ITEM_INDEX = 10_000;
const CHAT_COMPOSER_CLEARANCE = 24;
// Visual bottom stays intentionally strict so the FAB only hides when the
// viewport is truly pinned.
const VISUAL_BOTTOM_THRESHOLD = 4;
// Treat three explicit upward scroll gestures as "the user is reading history,
// stop force-following the latest stream". The test harness uses ~12px per
// gesture, so 36px is the practical threshold here.
const BOTTOM_FOLLOW_RELEASE_SCROLL_DISTANCE = 36;
// Ignore scroll events caused by our own bottom-forcing scroll for one short
// window so programmatic movement does not look like user intent.
const SELF_SCROLL_IGNORE_WINDOW_MS = 160;

type BottomAlignmentPhase =
  | 'idle'
  | 'requesting'
  | 'verifying'
  | 'aligned'
  | 'aborted';

export function getPrependedFirstItemIndex(
  current: number,
  prependCount: number,
): number {
  return Math.max(0, current - prependCount);
}

export function getInitialTopMostItemIndex(
  firstItemIndex: number,
  itemCount: number,
): IndexLocationWithAlign | number {
  return itemCount > 0
    ? {
        index: firstItemIndex + itemCount - 1,
        align: 'end',
      }
    : firstItemIndex;
}

export function getVisualBottomThreshold(): number {
  return VISUAL_BOTTOM_THRESHOLD;
}

export function shouldShowAnalysisLoader(
  latestMessage: Message | undefined,
  workflowStatus: ReturnType<typeof useAgentChat>['workflowStatus'],
): boolean {
  return (
    workflowStatus === 'busy' &&
    (latestMessage?.role !== 'assistant' ||
      (latestMessage?.role === 'assistant' &&
        !latestMessage.content?.length &&
        !latestMessage.thinking &&
        !latestMessage.tool_calls?.length))
  );
}

export function isPinnedToBottom(
  distanceFromBottom: number,
  threshold = VISUAL_BOTTOM_THRESHOLD,
): boolean {
  return distanceFromBottom <= threshold;
}

function isBottomAlignmentActive(phase: BottomAlignmentPhase): boolean {
  return phase === 'requesting' || phase === 'verifying';
}

function setForwardedRef<T>(ref: ForwardedRef<T>, value: T) {
  if (typeof ref === 'function') {
    ref(value);
    return;
  }

  if (ref) {
    ref.current = value;
  }
}

function scrollFooterSentinelIntoView(sentinel: HTMLDivElement | null) {
  // Test doubles can replace the DOM node with a partial mock that lacks the
  // real method, so keep the runtime guard instead of assuming browser-only DOM.
  if (!sentinel || typeof sentinel.scrollIntoView !== 'function') {
    return;
  }

  sentinel.scrollIntoView({
    block: 'end',
    inline: 'nearest',
    behavior: 'auto',
  });
}

function scrollVirtuosoToBottom(
  virtuoso: VirtuosoHandle | null,
  itemCount: number,
): boolean {
  if (!virtuoso || itemCount === 0) {
    return false;
  }

  virtuoso.scrollToIndex({
    index: 'LAST',
    align: 'end',
    behavior: 'auto',
  });

  return true;
}

function renderVirtualPlaceholder() {
  return <div aria-hidden="true" className="h-px" />;
}

function getScrollContentElement(
  scroller: HTMLDivElement | null,
): HTMLElement | null {
  const firstChild = scroller?.firstElementChild;
  return firstChild instanceof HTMLElement ? firstChild : null;
}

interface AgentChatVirtuosoContext {
  agentError: ReturnType<typeof useAgentChat>['error'];
  agentLlmError: ReturnType<typeof useAgentChat>['llmError'];
  footerEndRef: MutableRefObject<HTMLDivElement | null>;
  hasOlderMessages: boolean;
  isLoadingOlderMessages: boolean;
  latestMessage: Message | undefined;
  loadingOlderLabel: string;
  pendingApprovals: ReturnType<typeof useAgentSession>['pendingApprovals'];
  respondToToolApproval: ReturnType<
    typeof useAgentSession
  >['respondToToolApproval'];
  retryMessage: ReturnType<typeof useAgentChat>['retryMessage'];
  scrollToLoadOlderLabel: string;
  sessionAssistantName: string;
  workflowStatus: ReturnType<typeof useAgentChat>['workflowStatus'];
  yoloModeEnabled: ReturnType<typeof useAgentSession>['yoloModeEnabled'];
}

type AgentChatVirtuosoContextProps = {
  context: AgentChatVirtuosoContext;
};

const AgentChatMessagesList = forwardRef<
  HTMLDivElement,
  ListProps & AgentChatVirtuosoContextProps
>(function AgentChatMessagesList({ children, context, style, ...props }, ref) {
  void context;
  return (
    <div
      {...props}
      ref={ref}
      style={{
        ...style,
        paddingLeft: '16px',
        paddingRight: '16px',
      }}
    >
      {children}
    </div>
  );
});

function AgentChatMessagesHeader({ context }: AgentChatVirtuosoContextProps) {
  if (!context.hasOlderMessages && !context.isLoadingOlderMessages) {
    return null;
  }

  return (
    <div className="flex justify-center px-4">
      <div className="rounded-full border border-border/60 bg-background/80 px-3 py-1 text-xs text-muted-foreground shadow-sm">
        {context.isLoadingOlderMessages
          ? context.loadingOlderLabel
          : context.scrollToLoadOlderLabel}
      </div>
    </div>
  );
}

function AgentChatMessagesFooter({ context }: AgentChatVirtuosoContextProps) {
  const latestMessage = context.latestMessage;
  const showAnalysisLoader = shouldShowAnalysisLoader(
    latestMessage,
    context.workflowStatus,
  );

  return (
    <div className="px-4">
      {context.agentError && (
        <div className="self-start mt-2">
          <ErrorBubble
            error={context.agentError}
            onRetry={context.retryMessage}
          />
        </div>
      )}

      {context.agentLlmError && (
        <div className="self-start mt-2">
          <ErrorBubble
            error={context.agentLlmError}
            onRetry={context.retryMessage}
          />
        </div>
      )}

      {showAnalysisLoader && (
        <div className="flex justify-start mb-8 mt-3">
          <div className="w-full max-w-full bg-secondary/30 rounded-lg px-6 py-5">
            <div className="flex items-center gap-3 mb-2">
              <div className="w-7 h-7 bg-primary rounded-full flex items-center justify-center animate-pulse">
                <Bot size={16} className="text-primary-foreground" />
              </div>
              <span className="text-xs font-medium">
                {context.sessionAssistantName}
              </span>
            </div>
            <div className="text-sm">
              <AnalysisLoader size="md" />
            </div>
          </div>
        </div>
      )}

      {context.pendingApprovals && context.pendingApprovals.length > 0 && (
        <div className="flex justify-start mb-8 mt-3">
          <PendingApprovalWidget
            approvals={context.pendingApprovals}
            yoloModeEnabled={context.yoloModeEnabled}
            onRespond={context.respondToToolApproval}
          />
        </div>
      )}

      <div
        aria-hidden="true"
        style={{
          height: `calc(var(--agent-chat-composer-overlap, 64px) + ${CHAT_COMPOSER_CLEARANCE}px)`,
        }}
      />
      <div
        ref={(node) => {
          context.footerEndRef.current = node;
        }}
        aria-hidden="true"
        className="h-px w-full shrink-0"
      />
    </div>
  );
}

function truncatePreview(value: string, maxLength = 96): string {
  const trimmed = value.replace(/\s+/g, ' ').trim();

  if (!trimmed) {
    return '';
  }

  if (trimmed.length <= maxLength) {
    return trimmed;
  }

  return `${trimmed.slice(0, maxLength - 1).trimEnd()}…`;
}

function extractMessagePreview(
  message: Message | undefined,
): string | undefined {
  if (!message) {
    return undefined;
  }

  for (const item of message.content) {
    if (item.type === 'text') {
      const preview = truncatePreview(item.text);
      if (preview) {
        return preview;
      }
    }

    if (item.type === 'thinking') {
      const preview = truncatePreview(item.thinking);
      if (preview) {
        return preview;
      }
    }

    if (item.type === 'tool_call') {
      return truncatePreview(`Tool call: ${item.name}`);
    }
  }

  if (message.tool_calls?.length) {
    return truncatePreview(
      `Tool call: ${message.tool_calls[0]?.function.name}`,
    );
  }

  if (message.role === 'tool') {
    return 'Tool result';
  }

  return undefined;
}

function groupedMessageContainsBoundary(
  groupedMessage: GroupedMessage,
  boundaryId: string | undefined,
): boolean {
  if (!boundaryId) {
    return false;
  }

  if (groupedMessage.type === 'single') {
    return groupedMessage.message.id === boundaryId;
  }

  if (groupedMessage.type === 'tool_group') {
    return groupedMessage.coveredMessageIds.includes(boundaryId);
  }

  return groupedMessage.messages.some((message) => message.id === boundaryId);
}

function findGroupedMessageIndexByBoundary(
  groupedMessages: GroupedMessage[],
  boundaryId: string | undefined,
): number {
  if (!boundaryId) {
    return -1;
  }

  return groupedMessages.findIndex((groupedMessage) =>
    groupedMessageContainsBoundary(groupedMessage, boundaryId),
  );
}

export function getGroupedMessageVirtuosoKey(
  groupedMessage: GroupedMessage,
): string {
  if (groupedMessage.type === 'tool_group') {
    const firstCoveredId = groupedMessage.coveredMessageIds[0] ?? 'none';
    const lastCoveredId =
      groupedMessage.coveredMessageIds[
        groupedMessage.coveredMessageIds.length - 1
      ] ?? firstCoveredId;

    return [
      'tool-group',
      groupedMessage.message.id,
      firstCoveredId,
      lastCoveredId,
      groupedMessage.coveredMessageIds.length,
      groupedMessage.toolGroup.calls.length,
    ].join(':');
  }

  if (groupedMessage.type === 'tool_error_group') {
    const lastMessageId =
      groupedMessage.messages[groupedMessage.messages.length - 1]?.id ??
      groupedMessage.message.id;

    return [
      'tool-error-group',
      groupedMessage.message.id,
      lastMessageId,
      groupedMessage.messages.length,
    ].join(':');
  }

  return `single:${groupedMessage.message.id}`;
}

export function AgentChatMessages() {
  const { t } = useTranslation();
  const {
    messages,
    pendingMessages,
    error,
    llmError,
    retryMessage,
    workflowStatus,
  } = useAgentChat();
  const {
    session,
    pendingApprovals,
    respondToToolApproval,
    yoloModeEnabled,
    hasOlderMessages,
    isLoadingOlderMessages,
    loadOlderMessages,
  } = useAgentSession();
  const { getCompactedRange } = useLLMService();

  // Compact range for divider rendering (null if no compaction has occurred)
  const compactedRange = session?.id
    ? getCompactedRange(session.id)
    : undefined;
  const { refetchSessionFiles } = useAgentResourceAttachment();

  function handleReachTop() {
    if (!hasOlderMessages || isLoadingOlderMessages) {
      return;
    }

    void loadOlderMessages().catch((err) => {
      logger.error('Failed to load older messages after scroll trigger', err);
    });
  }

  useFileRefetcher({ messages, refetchSessionFiles });

  // Group messages for display
  const { groupedMessages, toolResultsMap } = useMessageGrouping(messages);

  // Convert pendingMessages to a Set of IDs for O(1) lookups
  // This prevents O(n*m) performance issues when checking if each message is pending
  const pendingMessageIds = useMemo(
    () => new Set(pendingMessages.map((msg) => msg.id)),
    [pendingMessages],
  );

  const latestMessage = messages[messages.length - 1];

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const assistantName = session?.assistant?.name || 'Agent';
  const footerEndRef = useRef<HTMLDivElement | null>(null);
  const scrollerElementRef = useRef<HTMLDivElement | null>(null);
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
  const currentFirstGroupedMessageId = groupedMessages[0]?.message.id;
  const currentLastGroupedMessageId =
    groupedMessages[groupedMessages.length - 1]?.message.id;
  const didSessionChangeForListState =
    previousListState.sessionId !== session?.id;
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
    sessionId: session?.id,
    firstItemIndex,
    effectiveFirstItemIndex,
  });
  const initialTopMostItemIndexRef = useRef<
    ReturnType<typeof getInitialTopMostItemIndex>
  >(
    getInitialTopMostItemIndex(
      INITIAL_FIRST_ITEM_INDEX,
      groupedMessages.length,
    ),
  );
  const initialTopMostItemSessionIdRef = useRef(session?.id);
  if (initialTopMostItemSessionIdRef.current !== session?.id) {
    initialTopMostItemSessionIdRef.current = session?.id;
    initialTopMostItemIndexRef.current = getInitialTopMostItemIndex(
      INITIAL_FIRST_ITEM_INDEX,
      groupedMessages.length,
    );
  }
  scrollDebugStateRef.current = {
    sessionId: session?.id,
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
      sessionId: session?.id,
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
        hasScroller: !!scrollerElementRef.current,
        hasFooterSentinel: !!footerEndRef.current,
        ...extra,
      });
    },
    [],
  );

  const compactedEvent = useMemo(() => {
    if (!compactedRange) {
      return undefined;
    }

    const fromIndex = messages.findIndex(
      (message) => message.id === compactedRange.fromId,
    );
    const toIndex = messages.findIndex(
      (message) => message.id === compactedRange.toId,
    );

    if (toIndex === -1) {
      return undefined;
    }

    if (fromIndex > toIndex) {
      return undefined;
    }

    return {
      earlierPreview:
        fromIndex === -1
          ? undefined
          : extractMessagePreview(messages[fromIndex]),
      latestIncludedPreview: extractMessagePreview(messages[toIndex]),
      condensedCount: fromIndex === -1 ? undefined : toIndex - fromIndex + 1,
      summary: compactedRange.summary,
    };
  }, [compactedRange, messages]);

  // Memoize references so ErrorBubble memo stays effective during streaming re-renders
  const agentError = useMemo(() => error, [error]);
  const agentLlmError = useMemo(() => llmError, [llmError]);
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
        nextSessionId: session?.id,
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
      sessionId: session?.id,
    };
  }, [
    currentFirstGroupedMessageId,
    currentLastGroupedMessageId,
    didSessionChangeForListState,
    effectiveFirstItemIndex,
    groupedMessages.length,
    logScrollState,
    prependCount,
    previousListState.sessionId,
    session?.id,
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
    session?.id,
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
    session?.id,
    setEffectivePinnedState,
  ]);

  useEffect(() => {
    const trackedSessionId = hasHydratedMessagesRef.current.sessionId;

    if (trackedSessionId !== session?.id) {
      hasHydratedMessagesRef.current = {
        sessionId: session?.id,
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
    session?.id,
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
    session?.id,
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
    session?.id,
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

  const virtuosoContext = useMemo<AgentChatVirtuosoContext>(
    () => ({
      agentError,
      agentLlmError,
      footerEndRef,
      hasOlderMessages,
      isLoadingOlderMessages,
      latestMessage,
      loadingOlderLabel: t(
        'agent.messages.loadingOlder',
        'Loading older messages...',
      ),
      pendingApprovals,
      respondToToolApproval,
      retryMessage,
      scrollToLoadOlderLabel: t(
        'agent.messages.scrollToLoadOlder',
        'Scroll up to load older messages',
      ),
      sessionAssistantName: assistantName,
      workflowStatus,
      yoloModeEnabled,
    }),
    [
      agentError,
      agentLlmError,
      footerEndRef,
      hasOlderMessages,
      isLoadingOlderMessages,
      latestMessage,
      t,
      pendingApprovals,
      respondToToolApproval,
      retryMessage,
      assistantName,
      workflowStatus,
      yoloModeEnabled,
    ],
  );

  const virtuosoComponents = useMemo<
    Components<GroupedMessage, AgentChatVirtuosoContext>
  >(() => {
    const AgentChatMessagesScroller = forwardRef<
      HTMLDivElement,
      ComponentPropsWithoutRef<'div'>
    >(function AgentChatMessagesScroller({ className, style, ...props }, ref) {
      return (
        <div
          {...props}
          ref={(node) => {
            scrollerElementRef.current = node;
            setScrollerElement((current) =>
              current === node ? current : node,
            );
            logScrollState('scroller-ref:set', {
              hasNode: !!node,
            });
            setForwardedRef(ref, node);
          }}
          className={cn('agent-chat-scrollbar', className)}
          style={{
            ...style,
            overflowAnchor: 'none',
          }}
        />
      );
    });

    return {
      Footer: AgentChatMessagesFooter,
      Header: AgentChatMessagesHeader,
      List: AgentChatMessagesList,
      Scroller: AgentChatMessagesScroller,
    };
  }, [logScrollState]);

  const renderMessageGroup = useCallback(
    (_index: number, groupedMessage: GroupedMessage) => {
      const isCompactBoundary = groupedMessageContainsBoundary(
        groupedMessage,
        compactedRange?.toId,
      );

      const compactDivider = isCompactBoundary ? (
        <CompactEventDivider
          key={`compact-divider-${groupedMessage.message.id}`}
          earlierPreview={compactedEvent?.earlierPreview}
          latestIncludedPreview={compactedEvent?.latestIncludedPreview}
          condensedCount={compactedEvent?.condensedCount}
          summary={compactedEvent?.summary}
        />
      ) : null;

      if (groupedMessage.type === 'tool_group') {
        return (
          <div className="mb-6">
            <AgentMessageBubble
              message={groupedMessage.message}
              assistantName={assistantName}
              toolResultsMap={toolResultsMap}
              groupedToolCalls={groupedMessage.toolGroup.calls}
              groupedMessages={groupedMessage.messages}
              isPending={pendingMessageIds.has(groupedMessage.message.id)}
            />
            {compactDivider}
          </div>
        );
      }

      if (groupedMessage.type === 'tool_error_group') {
        return (
          <div className="mb-6">
            <AgentMessageBubble
              message={groupedMessage.message}
              assistantName={assistantName}
              groupedMessages={groupedMessage.messages}
              isPending={pendingMessageIds.has(groupedMessage.message.id)}
              toolErrorGroup={true}
            />
            {compactDivider}
          </div>
        );
      }

      if (groupedMessage.message.error) {
        return (
          <div className="mb-6">
            <div className="self-start my-2">
              <ErrorBubble
                error={groupedMessage.message.error}
                onRetry={retryMessage}
              />
            </div>
            {compactDivider}
          </div>
        );
      }

      const msg = groupedMessage.message;
      const hasContent = msg?.content && msg.content.length > 0;
      const hasThinking = !!msg?.thinking;
      const hasToolCalls = msg?.tool_calls && msg.tool_calls.length > 0;

      if (
        !msg ||
        (!hasContent &&
          !hasThinking &&
          !hasToolCalls &&
          workflowStatus === 'busy')
      ) {
        return renderVirtualPlaceholder();
      }

      return (
        <div className="mb-6">
          <AgentMessageBubble
            message={msg}
            assistantName={assistantName}
            isPending={pendingMessageIds.has(msg.id)}
          />
          {compactDivider}
        </div>
      );
    },
    [
      assistantName,
      compactedEvent?.condensedCount,
      compactedEvent?.earlierPreview,
      compactedEvent?.latestIncludedPreview,
      compactedEvent?.summary,
      compactedRange?.toId,
      pendingMessageIds,
      retryMessage,
      toolResultsMap,
      workflowStatus,
    ],
  );

  return (
    <div className="relative flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
      <Virtuoso
        key={session?.id ?? 'agent-chat'}
        ref={virtuosoRef}
        className="flex-1"
        style={{ height: '100%' }}
        data={groupedMessages}
        components={virtuosoComponents}
        computeItemKey={(_, groupedMessage) =>
          getGroupedMessageVirtuosoKey(groupedMessage)
        }
        context={virtuosoContext}
        firstItemIndex={effectiveFirstItemIndex}
        initialTopMostItemIndex={initialTopMostItemIndexRef.current}
        atBottomThreshold={bottomThreshold}
        atBottomStateChange={handleVirtuosoAtBottomStateChange}
        // Disabled: the latch logic is the sole owner of bottom-follow behavior.
        followOutput={false}
        increaseViewportBy={{ top: 640, bottom: 960 }}
        startReached={handleReachTop}
        totalListHeightChanged={handleTotalListHeightChanged}
        itemContent={renderMessageGroup}
      />
      {!isPinned && (
        <div
          className="pointer-events-none absolute right-6 z-10"
          style={{
            bottom: `calc(var(--agent-chat-composer-overlap, 64px) + ${CHAT_COMPOSER_CLEARANCE + 16}px)`,
          }}
        >
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                size="icon"
                className="pointer-events-auto size-10 rounded-full shadow-lg"
                aria-label={t(
                  'agent.messages.scrollToLatest',
                  'Scroll to latest',
                )}
                onClick={handleManualScrollToBottom}
              >
                <ChevronDown className="size-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.messages.scrollToLatest', 'Scroll to latest')}
            </TooltipContent>
          </Tooltip>
        </div>
      )}
    </div>
  );
}
