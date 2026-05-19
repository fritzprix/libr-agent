import {
  type ComponentPropsWithoutRef,
  type ForwardedRef,
  type MutableRefObject,
  forwardRef,
  useCallback,
  useEffect,
  useLayoutEffect,
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
// viewport is truly pinned, while the streaming latch absorbs normal drift.
const VISUAL_BOTTOM_THRESHOLD = 4;
// A stream that is still within ~3 lines of the bottom should snap back into
// the latch when the user scrolls downward again.
const STREAMING_LATCH_ACQUIRE_DISTANCE = 48;
// Small upward movement away from bottom should be enough to escape the latch.
const STREAMING_LATCH_DIRECTIONAL_RELEASE_DISTANCE = 24;
// A large distance from bottom always releases the latch, regardless of delta.
const STREAMING_LATCH_RELEASE_DISTANCE = 120;
// Ignore tiny wheel/touch jitter; require a real upward gesture before release.
const STREAMING_LATCH_MIN_UPWARD_DELTA = 12;
// Keep the latch briefly after busy->idle so the final token/layout settle
// doesn't immediately drop the viewport.
const STREAMING_LATCH_SETTLE_MS = 160;
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

function isNearBottomForLatch(distanceFromBottom: number): boolean {
  return distanceFromBottom <= STREAMING_LATCH_ACQUIRE_DISTANCE;
}

function isBottomAlignmentActive(phase: BottomAlignmentPhase): boolean {
  return phase === 'requesting' || phase === 'verifying';
}

// User intent wins over our own scroll-to-bottom bookkeeping: a deliberate
// upward gesture should always release the latch, while programmatic scrolls
// only keep the latch alive until the user takes over.
function shouldReleaseBottomLatch(
  distanceFromBottom: number,
  scrollDelta: number,
  isProgrammaticScroll: boolean,
): boolean {
  const hasUpwardReleaseGesture =
    scrollDelta <= -STREAMING_LATCH_MIN_UPWARD_DELTA &&
    distanceFromBottom > STREAMING_LATCH_DIRECTIONAL_RELEASE_DISTANCE;

  if (hasUpwardReleaseGesture) {
    return true;
  }

  if (isProgrammaticScroll) {
    return false;
  }

  return distanceFromBottom > STREAMING_LATCH_RELEASE_DISTANCE;
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
  const bottomLatchActiveRef = useRef(false);
  const bottomAlignmentPhaseRef = useRef<BottomAlignmentPhase>('idle');
  const bottomAlignmentLayoutVersionRef = useRef(0);
  const bottomAlignmentRequestedVersionRef = useRef(0);
  const bottomAlignmentVerifyFrameRef = useRef<number | null>(null);
  const bottomLatchSettleTimeoutRef = useRef<number | null>(null);
  const prependStabilizeTimeoutRef = useRef<number | null>(null);
  const isPreservingPrependPositionRef = useRef(false);
  const autoScrollFrameRef = useRef<number | null>(null);
  const selfScrollIgnoreUntilRef = useRef(0);
  const previousScrollTopRef = useRef<number | null>(null);
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
  const effectiveFirstItemIndex =
    previousListStateRef.current.sessionId === session?.id
      ? firstItemIndex
      : INITIAL_FIRST_ITEM_INDEX;
  const scrollDebugStateRef = useRef({
    sessionId: session?.id,
    firstItemIndex,
    effectiveFirstItemIndex,
  });
  scrollDebugStateRef.current = {
    sessionId: session?.id,
    firstItemIndex,
    effectiveFirstItemIndex,
  };
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
        bottomLatchActive: bottomLatchActiveRef.current,
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

  const clearBottomLatchSettleTimeout = useCallback(() => {
    if (bottomLatchSettleTimeoutRef.current !== null) {
      window.clearTimeout(bottomLatchSettleTimeoutRef.current);
      bottomLatchSettleTimeoutRef.current = null;
    }
  }, []);

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

  const acquireBottomLatch = useCallback(
    (reason: string) => {
      clearBottomLatchSettleTimeout();
      if (bottomLatchActiveRef.current) {
        return;
      }

      bottomLatchActiveRef.current = true;
      setEffectivePinnedState(true);
      logScrollState('bottom-latch:acquire', {
        reason,
      });
    },
    [clearBottomLatchSettleTimeout, logScrollState, setEffectivePinnedState],
  );

  const releaseBottomLatch = useCallback(
    (reason: string) => {
      clearBottomLatchSettleTimeout();
      if (!bottomLatchActiveRef.current) {
        return;
      }

      bottomLatchActiveRef.current = false;
      setEffectivePinnedState(visualBottomRef.current);
      logScrollState('bottom-latch:release', {
        reason,
        visualBottom: visualBottomRef.current,
      });
    },
    [clearBottomLatchSettleTimeout, logScrollState, setEffectivePinnedState],
  );

  const executeScrollToBottom = useCallback(
    (reason: string): boolean => {
      const itemCount = groupedMessageCountRef.current;
      selfScrollIgnoreUntilRef.current =
        performance.now() + SELF_SCROLL_IGNORE_WINDOW_MS;
      logScrollState('executeScrollToBottom:start', {
        itemCount,
        reason,
        bottomLatchActive: bottomLatchActiveRef.current,
      });

      if (footerEndRef.current) {
        logScrollState('executeScrollToBottom:footer-sentinel', {
          itemCount,
          reason,
        });
        scrollFooterSentinelIntoView(footerEndRef.current);
        return true;
      }

      const scrolledWithVirtuoso = scrollVirtuosoToBottom(
        virtuosoRef.current,
        itemCount,
      );

      if (!scrolledWithVirtuoso) {
        logScrollState('executeScrollToBottom:unavailable', {
          itemCount,
          reason,
          scrolledWithVirtuoso,
        });
        return false;
      }

      logScrollState('executeScrollToBottom:virtuoso-scroll', {
        itemCount,
        reason,
        scrolledWithVirtuoso,
      });
      return true;
    },
    [logScrollState],
  );

  useLayoutEffect(() => {
    const previous = previousListStateRef.current;
    const firstId = groupedMessages[0]?.message.id;
    const lastId = groupedMessages[groupedMessages.length - 1]?.message.id;

    if (previous.sessionId !== session?.id) {
      logScrollState('listState:session-changed', {
        previousSessionId: previous.sessionId,
        nextSessionId: session?.id,
        groupedMessageCount: groupedMessages.length,
      });
      setFirstItemIndex(INITIAL_FIRST_ITEM_INDEX);
    } else if (
      groupedMessages.length > previous.length &&
      previous.lastId === lastId &&
      previous.firstId !== firstId
    ) {
      const prependCount = groupedMessages.length - previous.length;
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
      setFirstItemIndex((current) =>
        getPrependedFirstItemIndex(current, prependCount),
      );
    }

    previousListStateRef.current = {
      firstId,
      lastId,
      length: groupedMessages.length,
      sessionId: session?.id,
    };
  }, [groupedMessages, logScrollState, session?.id]);

  const scheduleScrollToBottom = useCallback(
    (reason: string) => {
      const shouldForce =
        forceBottomScrollReasons.has(reason) || bottomLatchActiveRef.current;
      const shouldSuppressForPrepend =
        isPreservingPrependPositionRef.current &&
        !forceBottomScrollReasons.has(reason);

      const shouldSuppressForUserScroll =
        !isBottomAlignmentActive(bottomAlignmentPhaseRef.current) &&
        !visualBottomRef.current &&
        !shouldForce;

      if (shouldSuppressForPrepend || shouldSuppressForUserScroll) {
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
      if (autoScrollFrameRef.current !== null) {
        cancelAnimationFrame(autoScrollFrameRef.current);
      }

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
    bottomLatchActiveRef.current = false;
    isPreservingPrependPositionRef.current = false;
    setEffectivePinnedState(true);
    if (autoScrollFrameRef.current !== null) {
      cancelAnimationFrame(autoScrollFrameRef.current);
      autoScrollFrameRef.current = null;
    }
    if (prependStabilizeTimeoutRef.current !== null) {
      window.clearTimeout(prependStabilizeTimeoutRef.current);
      prependStabilizeTimeoutRef.current = null;
    }
    clearBottomLatchSettleTimeout();
    previousScrollTopRef.current = null;
    logScrollState('sessionEffect:reset-bottom-alignment');
    requestBottomAlignment('session-changed');
    scheduleScrollToBottom('session-changed');
  }, [
    clearBottomAlignmentVerifyFrame,
    clearBottomLatchSettleTimeout,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    session?.id,
    setEffectivePinnedState,
    setBottomAlignmentPhase,
  ]);

  useEffect(() => {
    return () => {
      if (autoScrollFrameRef.current !== null) {
        cancelAnimationFrame(autoScrollFrameRef.current);
        autoScrollFrameRef.current = null;
      }
      if (prependStabilizeTimeoutRef.current !== null) {
        window.clearTimeout(prependStabilizeTimeoutRef.current);
        prependStabilizeTimeoutRef.current = null;
      }
      clearBottomAlignmentVerifyFrame();
      clearBottomLatchSettleTimeout();
    };
  }, [clearBottomAlignmentVerifyFrame, clearBottomLatchSettleTimeout]);

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
      const nearBottomForLatch = isNearBottomForLatch(distanceFromBottom);
      const isSelfScroll = performance.now() < selfScrollIgnoreUntilRef.current;

      visualBottomRef.current = visualPinned;

      if (
        bottomLatchActiveRef.current &&
        !isPreservingPrependPositionRef.current &&
        shouldReleaseBottomLatch(distanceFromBottom, scrollDelta, isSelfScroll)
      ) {
        abortBottomAlignment('scroll-release');
        if (autoScrollFrameRef.current !== null) {
          cancelAnimationFrame(autoScrollFrameRef.current);
          autoScrollFrameRef.current = null;
        }
        releaseBottomLatch('scroll-release');
      }

      if (
        !bottomLatchActiveRef.current &&
        workflowStatus === 'busy' &&
        nearBottomForLatch &&
        !isSelfScroll &&
        scrollDelta > 0 &&
        !isPreservingPrependPositionRef.current
      ) {
        acquireBottomLatch('scroll-reacquire');
      }

      if (!visualPinned && !bottomLatchActiveRef.current) {
        virtuosoAtBottomRef.current = false;
        abortBottomAlignment('visual-bottom-lost');
      }

      scheduleBottomAlignmentVerification('scroll:updatePinnedState');

      setEffectivePinnedState(
        bottomLatchActiveRef.current ? true : visualPinned,
      );
      logScrollState('scroll:updatePinnedState', {
        currentScrollTop,
        scrollDelta,
        distanceFromBottom,
        visualPinned,
        nearBottomForLatch,
        isSelfScroll,
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
    acquireBottomLatch,
    abortBottomAlignment,
    bottomThreshold,
    logScrollState,
    releaseBottomLatch,
    scheduleBottomAlignmentVerification,
    scrollerElement,
    session?.id,
    setEffectivePinnedState,
    workflowStatus,
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
    requestBottomAlignment('manual-scroll-to-bottom');
    if (workflowStatus === 'busy') {
      acquireBottomLatch('manual-scroll-to-bottom');
    } else {
      setEffectivePinnedState(true);
    }
    logScrollState('scrollToBottom:manual');
    scheduleScrollToBottom('manual-scroll-to-bottom');
  }, [
    acquireBottomLatch,
    logScrollState,
    requestBottomAlignment,
    scheduleScrollToBottom,
    setEffectivePinnedState,
    workflowStatus,
  ]);

  useEffect(() => {
    const content = getScrollContentElement(scrollerElement);

    if (!content || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      markBottomAlignmentLayoutChanged('content-resize-observer');
      scheduleBottomAlignmentVerification('content-resize-observer');
      if (
        !bottomLatchActiveRef.current &&
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
      markBottomAlignmentLayoutChanged('scroller-resize-observer');
      scheduleBottomAlignmentVerification('scroller-resize-observer');
      if (
        !bottomLatchActiveRef.current &&
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
    scheduleBottomAlignmentVerification,
    scheduleScrollToBottom,
    scrollerElement,
    session?.id,
  ]);

  useEffect(() => {
    if (workflowStatus === 'busy') {
      if (
        !bottomLatchActiveRef.current &&
        !isPreservingPrependPositionRef.current &&
        groupedMessages.length > 0 &&
        hasHydratedMessagesRef.current.hasMessages &&
        (visualBottomRef.current ||
          isBottomAlignmentActive(bottomAlignmentPhaseRef.current))
      ) {
        acquireBottomLatch('workflow-busy');
      }
    } else if (bottomLatchActiveRef.current) {
      clearBottomLatchSettleTimeout();
      bottomLatchSettleTimeoutRef.current = window.setTimeout(() => {
        bottomLatchSettleTimeoutRef.current = null;
        releaseBottomLatch('workflow-settled');
      }, STREAMING_LATCH_SETTLE_MS);
    }

    if (!bottomLatchActiveRef.current && !isPinnedToBottomRef.current) {
      return;
    }

    scheduleScrollToBottom('reactive-state-change');
  }, [
    acquireBottomLatch,
    latestMessage,
    workflowStatus,
    pendingApprovals?.length,
    agentError,
    agentLlmError,
    groupedMessages.length,
    releaseBottomLatch,
    scheduleScrollToBottom,
    clearBottomLatchSettleTimeout,
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
        computeItemKey={(_, groupedMessage) => groupedMessage.message.id}
        context={virtuosoContext}
        firstItemIndex={effectiveFirstItemIndex}
        initialTopMostItemIndex={getInitialTopMostItemIndex(
          effectiveFirstItemIndex,
          groupedMessages.length,
        )}
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
