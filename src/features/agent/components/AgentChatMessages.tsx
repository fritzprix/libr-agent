import {
  createContext,
  useContext,
  type ComponentPropsWithoutRef,
  forwardRef,
  useCallback,
  useMemo,
} from 'react';
import { useTranslation } from 'react-i18next';
import { Virtuoso, type Components } from 'react-virtuoso';
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
import { CompactEventDivider } from './shared/CompactEventDivider';
import { ChevronDown, ShieldAlert } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

// Submodule imports
import {
  SCROLL_TO_LATEST_BUTTON_OFFSET,
  type AgentChatVirtuosoContext,
} from './agent-chat-messages/types';
import {
  getGroupedMessageVirtuosoKey,
  setForwardedRef,
  renderVirtualPlaceholder,
  groupedMessageContainsBoundary,
} from './agent-chat-messages/utils';
import { useAgentChatScroll } from './agent-chat-messages/hooks/useAgentChatScroll';
import {
  AgentChatMessagesList,
  AgentChatMessagesHeader,
  AgentChatMessagesFooter,
} from './agent-chat-messages/components/VirtuosoListComponents';

const ScrollerContext = createContext<{
  setScrollerElement: (node: HTMLDivElement | null) => void;
  logScrollState: (
    event: string,
    extra?: Record<string, boolean | number | string | undefined>,
  ) => void;
} | null>(null);

const AgentChatMessagesScroller = forwardRef<
  HTMLDivElement,
  ComponentPropsWithoutRef<'div'>
>(function AgentChatMessagesScroller({ className, style, ...props }, ref) {
  const context = useContext(ScrollerContext);
  return (
    <div
      {...props}
      ref={(node) => {
        if (context) {
          context.setScrollerElement(node);
          context.logScrollState('scroller-ref:set', {
            hasNode: !!node,
          });
        }
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

const VIRTUOSO_COMPONENTS: Components<
  GroupedMessage,
  AgentChatVirtuosoContext
> = {
  Footer: AgentChatMessagesFooter,
  Header: AgentChatMessagesHeader,
  List: AgentChatMessagesList,
  Scroller: AgentChatMessagesScroller,
};

const VIRTUOSO_INCREASE_VIEWPORT_BY = { top: 640, bottom: 960 } as const;

// Re-export public functions to preserve the stable external API of the file
export {
  shouldShowAnalysisLoader,
  getPrependedFirstItemIndex,
  getInitialTopMostItemIndex,
  getVisualBottomThreshold,
  isPinnedToBottom,
  getGroupedMessageVirtuosoKey,
} from './agent-chat-messages/utils';

export function AgentChatMessages() {
  const { t } = useTranslation();
  const { messages, error, llmError, retryMessage, workflowStatus } =
    useAgentChat();
  const {
    session,
    pendingApprovals,
    respondToToolApproval,
    executionMode,
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

  useFileRefetcher({ messages, refetchSessionFiles });

  // Group messages for display
  const { groupedMessages, toolResultsMap } = useMessageGrouping(
    messages,
    compactedRange?.toId,
  );

  const latestMessage = messages[messages.length - 1];

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const assistantName = session?.assistant?.name || 'Agent';

  const {
    virtuosoRef,
    footerEndRef,
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
  } = useAgentChatScroll({
    groupedMessages,
    sessionId: session?.id,
    latestMessage,
    workflowStatus,
    pendingApprovals,
    agentError: error,
    agentLlmError: llmError,
    isLoadingOlderMessages,
  });

  const handleReachTop = useCallback(() => {
    handleStartReached();
    if (!hasOlderMessages || isLoadingOlderMessages) {
      return;
    }

    void loadOlderMessages().catch(() => {
      // Swallowed: loadOlderMessages already handles and logs errors internally.
    });
  }, [
    handleStartReached,
    hasOlderMessages,
    isLoadingOlderMessages,
    loadOlderMessages,
  ]);

  const compactedEvent = useMemo(() => {
    if (!compactedRange) {
      return undefined;
    }

    let toIndex = -1;
    for (let i = messages.length - 1; i >= 0; i--) {
      const id = messages[i]?.id;
      if (id === compactedRange.toId) {
        toIndex = i;
        break;
      }
    }

    if (toIndex === -1) {
      return undefined;
    }

    return {
      latestIncludedPreview: compactedRange.latestIncludedPreview,
      condensedCount: compactedRange.condensedCount,
      summary: compactedRange.summary,
    };
  }, [compactedRange, messages]);

  const virtuosoContext = useMemo<AgentChatVirtuosoContext>(
    () => ({
      agentError: error,
      agentLlmError: llmError,
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
      executionMode,
    }),
    [
      error,
      llmError,
      hasOlderMessages,
      isLoadingOlderMessages,
      latestMessage,
      t,
      pendingApprovals,
      respondToToolApproval,
      retryMessage,
      assistantName,
      workflowStatus,
      executionMode,
    ],
  );

  const renderMessageGroup = useCallback(
    (_index: number, groupedMessage: GroupedMessage) => {
      // Only pin nested auto-scroll (thinking / resources) while the list itself
      // is stick-to-bottom. The old `!isLatest || isPinned` kept followChatScroll
      // true for every historical bubble, so load-older mounts could briefly
      // yank a user bubble into view then release it.
      const followChatScroll = isPinned;
      const isCompactBoundary = groupedMessageContainsBoundary(
        groupedMessage,
        compactedRange?.toId,
      );

      const compactDivider = isCompactBoundary ? (
        <CompactEventDivider
          key={`compact-divider-${groupedMessage.message.id}`}
          latestIncludedPreview={compactedEvent?.latestIncludedPreview}
          condensedCount={compactedEvent?.condensedCount}
          summary={compactedEvent?.summary}
        />
      ) : null;

      if (groupedMessage.type === 'tool_group') {
        return (
          // Use padding (not margin) for inter-item gap — Virtuoso's size
          // observer ignores vertical margins and mis-corrects scrollTop on
          // upward scroll, flashing above-viewport bubbles into view.
          <div className="pb-6">
            <AgentMessageBubble
              message={groupedMessage.message}
              assistantName={assistantName}
              toolResultsMap={toolResultsMap}
              groupedToolCalls={groupedMessage.toolGroup.calls}
              groupedMessages={groupedMessage.messages}
              followChatScroll={followChatScroll}
            />
            {compactDivider}
          </div>
        );
      }

      if (groupedMessage.type === 'tool_error_group') {
        return (
          <div className="pb-6">
            <AgentMessageBubble
              message={groupedMessage.message}
              assistantName={assistantName}
              groupedMessages={groupedMessage.messages}
              followChatScroll={followChatScroll}
              toolErrorGroup={true}
            />
            {compactDivider}
          </div>
        );
      }

      if (groupedMessage.message.error) {
        return (
          <div className="pb-6">
            <div className="self-start py-2">
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
        <div className="pb-6">
          <AgentMessageBubble
            message={msg}
            assistantName={assistantName}
            followChatScroll={followChatScroll}
          />
          {compactDivider}
        </div>
      );
    },
    [
      assistantName,
      compactedEvent,
      compactedRange?.toId,
      isPinned,
      retryMessage,
      toolResultsMap,
      workflowStatus,
    ],
  );

  const scrollerContextValue = useMemo(
    () => ({ setScrollerElement, logScrollState }),
    [setScrollerElement, logScrollState],
  );

  return (
    <ScrollerContext.Provider value={scrollerContextValue}>
      <div className="relative flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
        <Virtuoso
          key={session?.id ?? 'agent-chat'}
          ref={virtuosoRef}
          className="flex-1"
          style={{ height: '100%' }}
          data={groupedMessages}
          components={VIRTUOSO_COMPONENTS}
          computeItemKey={(_, groupedMessage) =>
            getGroupedMessageVirtuosoKey(groupedMessage)
          }
          context={virtuosoContext}
          firstItemIndex={effectiveFirstItemIndex}
          initialTopMostItemIndex={initialTopMostItemIndex}
          atBottomThreshold={bottomThreshold}
          atBottomStateChange={handleVirtuosoAtBottomStateChange}
          followOutput={false}
          // Chat starts at LAST via initialTopMostItemIndex. Without this,
          // scrolling up into never-measured variable-height rows briefly
          // paints those above-viewport bubbles then corrects (virtuoso#1096).
          skipAnimationFrameInResizeObserver
          increaseViewportBy={VIRTUOSO_INCREASE_VIEWPORT_BY}
          startReached={handleReachTop}
          totalListHeightChanged={handleTotalListHeightChanged}
          itemContent={renderMessageGroup}
        />
        {pendingApprovals.length > 0 && !isPinned ? (
          <div
            className="pointer-events-none absolute inset-x-0 top-3 z-20 flex justify-center px-4"
            style={{
              paddingRight: 'var(--agent-side-panel-inset, 0px)',
            }}
          >
            <Button
              type="button"
              size="sm"
              variant="secondary"
              className="pointer-events-auto gap-2 shadow-lg"
              onClick={handleManualScrollToBottom}
            >
              <ShieldAlert className="size-4 text-warning" />
              <span>
                {t('agent.messages.pendingApprovalsJump', {
                  count: pendingApprovals.length,
                  defaultValue:
                    '{{count}} tools awaiting approval — jump to respond',
                })}
              </span>
            </Button>
          </div>
        ) : null}
        {!isPinned && (
          <div
            className="pointer-events-none absolute z-20"
            style={{
              right: `calc(1.5rem + var(--agent-side-panel-inset, 0px))`,
              bottom: `calc(var(--agent-chat-composer-overlap, 64px) + ${SCROLL_TO_LATEST_BUTTON_OFFSET}px)`,
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
    </ScrollerContext.Provider>
  );
}
