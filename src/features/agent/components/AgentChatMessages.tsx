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
import { ChevronDown } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

// Submodule imports
import { type AgentChatVirtuosoContext } from './agent-chat-messages/types';
import {
  getGroupedMessageVirtuosoKey,
  setForwardedRef,
  renderVirtualPlaceholder,
  groupedMessageContainsBoundary,
  extractMessagePreview,
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

  const handleReachTop = useCallback(() => {
    if (!hasOlderMessages || isLoadingOlderMessages) {
      return;
    }

    void loadOlderMessages().catch(() => {
      // Swallowed: loadOlderMessages already handles and logs errors internally.
    });
  }, [hasOlderMessages, isLoadingOlderMessages, loadOlderMessages]);

  useFileRefetcher({ messages, refetchSessionFiles });

  // Group messages for display
  const { groupedMessages, toolResultsMap } = useMessageGrouping(messages);

  // Convert pendingMessages to a Set of IDs for O(1) lookups
  const pendingMessageIds = useMemo(
    () => new Set(pendingMessages.map((msg) => msg.id)),
    [pendingMessages],
  );

  const latestMessage = messages[messages.length - 1];

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const assistantName = session?.assistant?.name || 'Agent';

  // Instantiate our scroll, observers, and bottom alignment engine
  const {
    virtuosoRef,
    footerEndRef,
    setScrollerElement,
    effectiveFirstItemIndex,
    isPinned,
    handleVirtuosoAtBottomStateChange,
    handleTotalListHeightChanged,
    handleManualScrollToBottom,
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
  });

  const compactedEvent = useMemo(() => {
    if (!compactedRange) {
      return undefined;
    }

    let fromIndex = -1;
    let toIndex = -1;
    for (let i = messages.length - 1; i >= 0; i--) {
      const id = messages[i]?.id;
      if (id === compactedRange.toId) {
        toIndex = i;
      }
      if (id === compactedRange.fromId) {
        fromIndex = i;
      }
      if (fromIndex !== -1 && toIndex !== -1) {
        break;
      }
    }

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
      yoloModeEnabled,
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
      yoloModeEnabled,
    ],
  );

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
      compactedEvent,
      compactedRange?.toId,
      pendingMessageIds,
      retryMessage,
      toolResultsMap,
      workflowStatus,
    ],
  );

  return (
    <ScrollerContext.Provider value={{ setScrollerElement, logScrollState }}>
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
          increaseViewportBy={{ top: 640, bottom: 960 }}
          startReached={handleReachTop}
          totalListHeightChanged={handleTotalListHeightChanged}
          itemContent={renderMessageGroup}
        />
        {!isPinned && (
          <div
            className="pointer-events-none absolute right-6 z-10"
            style={{
              bottom: `calc(var(--agent-chat-composer-overlap, 64px) + 40px)`,
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
