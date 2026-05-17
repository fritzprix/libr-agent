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
const DEFAULT_BOTTOM_THRESHOLD = 4;
const CHAT_COMPOSER_CLEARANCE = 24;

export function getPrependedFirstItemIndex(
  current: number,
  prependCount: number,
): number {
  return Math.max(0, current - prependCount);
}

export function getInitialTopMostItemIndex(
  firstItemIndex: number,
  itemCount: number,
): number {
  return itemCount > 0 ? firstItemIndex + itemCount - 1 : firstItemIndex;
}

export function getVisualBottomThreshold(): number {
  return DEFAULT_BOTTOM_THRESHOLD;
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
  threshold = DEFAULT_BOTTOM_THRESHOLD,
): boolean {
  return distanceFromBottom <= threshold;
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
  sentinel?.scrollIntoView({
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
  const isPinnedToBottomRef = useRef(true);
  const autoScrollFrameRef = useRef<number | null>(null);
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
  const bottomThreshold = getVisualBottomThreshold();
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

  const scrollToBottomNow = useCallback(() => {
    const itemCount = groupedMessageCountRef.current;
    const scrolledWithVirtuoso = scrollVirtuosoToBottom(
      virtuosoRef.current,
      itemCount,
    );

    if (!scrolledWithVirtuoso || itemCount === 0) {
      scrollFooterSentinelIntoView(footerEndRef.current);
    }
  }, []);

  useEffect(() => {
    const previous = previousListStateRef.current;
    const firstId = groupedMessages[0]?.message.id;
    const lastId = groupedMessages[groupedMessages.length - 1]?.message.id;

    if (previous.sessionId !== session?.id) {
      setFirstItemIndex(INITIAL_FIRST_ITEM_INDEX);
    } else if (
      groupedMessages.length > previous.length &&
      previous.lastId === lastId &&
      previous.firstId !== firstId
    ) {
      const prependCount = groupedMessages.length - previous.length;
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
  }, [groupedMessages, session?.id]);

  const scheduleScrollToBottom = useCallback(() => {
    if (autoScrollFrameRef.current !== null) {
      cancelAnimationFrame(autoScrollFrameRef.current);
    }

    autoScrollFrameRef.current = requestAnimationFrame(() => {
      autoScrollFrameRef.current = null;
      scrollToBottomNow();
    });
  }, [scrollToBottomNow]);

  useEffect(() => {
    isPinnedToBottomRef.current = true;
    setIsPinned(true);
    if (autoScrollFrameRef.current !== null) {
      cancelAnimationFrame(autoScrollFrameRef.current);
      autoScrollFrameRef.current = null;
    }
    scheduleScrollToBottom();
  }, [scheduleScrollToBottom, session?.id]);

  useEffect(() => {
    return () => {
      if (autoScrollFrameRef.current !== null) {
        cancelAnimationFrame(autoScrollFrameRef.current);
        autoScrollFrameRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    const scroller = scrollerElementRef.current;

    if (!scroller) {
      return;
    }

    const updatePinnedState = () => {
      const distanceFromBottom =
        scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
      const nextPinned = isPinnedToBottom(distanceFromBottom, bottomThreshold);
      isPinnedToBottomRef.current = nextPinned;
      setIsPinned((current) => (current === nextPinned ? current : nextPinned));
    };
    const handleScroll = () => {
      updatePinnedState();
    };

    scroller.addEventListener('scroll', handleScroll, { passive: true });
    updatePinnedState();

    return () => {
      scroller.removeEventListener('scroll', handleScroll);
    };
  }, [bottomThreshold, session?.id]);

  useEffect(() => {
    const trackedSessionId = hasHydratedMessagesRef.current.sessionId;

    if (trackedSessionId !== session?.id) {
      hasHydratedMessagesRef.current = {
        sessionId: session?.id,
        hasMessages: groupedMessages.length > 0,
      };
      return;
    }

    if (
      !hasHydratedMessagesRef.current.hasMessages &&
      groupedMessages.length > 0 &&
      isPinnedToBottomRef.current
    ) {
      scheduleScrollToBottom();
    }

    if (groupedMessages.length > 0) {
      hasHydratedMessagesRef.current.hasMessages = true;
    }
  }, [groupedMessages.length, scheduleScrollToBottom, session?.id]);

  const scrollToBottom = useCallback(() => {
    isPinnedToBottomRef.current = true;
    setIsPinned(true);
    scheduleScrollToBottom();
  }, [scheduleScrollToBottom]);

  useEffect(() => {
    const scroller = scrollerElementRef.current;
    const content = getScrollContentElement(scroller);

    if (!content || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (!isPinnedToBottomRef.current) {
        return;
      }

      scheduleScrollToBottom();
    });

    observer.observe(content);

    return () => {
      observer.disconnect();
    };
  }, [scheduleScrollToBottom, session?.id]);

  useEffect(() => {
    const scroller = scrollerElementRef.current;

    if (!scroller || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(() => {
      if (!isPinnedToBottomRef.current) {
        return;
      }

      scheduleScrollToBottom();
    });

    observer.observe(scroller);

    return () => {
      observer.disconnect();
    };
  }, [scheduleScrollToBottom, session?.id]);

  useEffect(() => {
    if (!isPinnedToBottomRef.current) {
      return;
    }

    scheduleScrollToBottom();
  }, [
    latestMessage,
    workflowStatus,
    pendingApprovals?.length,
    agentError,
    agentLlmError,
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
  }, []);

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
        firstItemIndex={firstItemIndex}
        initialTopMostItemIndex={getInitialTopMostItemIndex(
          firstItemIndex,
          groupedMessages.length,
        )}
        atBottomThreshold={bottomThreshold}
        followOutput={false}
        increaseViewportBy={{ top: 640, bottom: 960 }}
        startReached={handleReachTop}
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
                onClick={scrollToBottom}
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
