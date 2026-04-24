import React, { useMemo } from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
import { useLLMService } from '@/context/LLMServiceContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useChatScroll } from '@/features/agent/hooks/useChatScroll';
import { useFileRefetcher } from '@/features/agent/hooks/useFileRefetcher';
import {
  useMessageGrouping,
  type GroupedMessage,
} from '@/hooks/useMessageGrouping';
import { AgentMessageBubble } from './AgentMessageBubble';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from './shared';
import { CompactEventDivider } from './shared/CompactEventDivider';
import { Bot } from 'lucide-react';
import { PendingApprovalWidget } from './PendingApprovalWidget';
import { ScrollArea } from '@/components/ui';
import { getLogger } from '@/lib/logger';
import type { Message } from '@/models/chat';

const logger = getLogger('AgentChatMessages');

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

    prepareForPrepend();
    void loadOlderMessages().catch((err) => {
      logger.error('Failed to load older messages after scroll trigger', err);
    });
  }

  // Use custom hooks for side effects
  const { messagesEndRef, scrollContainerRef, contentRef, prepareForPrepend } =
    useChatScroll({
      messages,
      onReachTop: handleReachTop,
      canLoadOlder: hasOlderMessages,
      isLoadingOlder: isLoadingOlderMessages,
    });
  useFileRefetcher({ messages, refetchSessionFiles });

  // Group messages for display
  const { groupedMessages, toolResultsMap } = useMessageGrouping(messages);

  // Convert pendingMessages to a Set of IDs for O(1) lookups
  // This prevents O(n*m) performance issues when checking if each message is pending
  const pendingMessageIds = useMemo(
    () => new Set(pendingMessages.map((msg) => msg.id)),
    [pendingMessages],
  );

  const lastMessageWho = messages[messages.length - 1]?.role;

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const assistantName = session?.assistant?.name || 'Agent';

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

    if (fromIndex === -1 || toIndex === -1 || fromIndex > toIndex) {
      return undefined;
    }

    return {
      earlierPreview: extractMessagePreview(messages[fromIndex]),
      latestIncludedPreview: extractMessagePreview(messages[toIndex]),
      condensedCount: toIndex - fromIndex + 1,
      summary: compactedRange.summary,
    };
  }, [compactedRange, messages]);

  // Memoize references so ErrorBubble memo stays effective during streaming re-renders
  const agentError = useMemo(() => error, [error]);
  const agentLlmError = useMemo(() => llmError, [llmError]);

  const streamingToolMessageIds = useMemo(
    () =>
      messages
        .filter(
          (message) =>
            message.role === 'assistant' &&
            message.isStreaming &&
            !!message.tool_calls?.length,
        )
        .map((message) => ({
          id: message.id,
          contentTypes: message.content?.map((item) => item.type) ?? [],
          toolCallCount: message.tool_calls?.length ?? 0,
        })),
    [messages],
  );

  if (streamingToolMessageIds.length > 0) {
    logger.info('AgentChatMessages received streaming tool messages', {
      sessionId: session?.id,
      messageCount: messages.length,
      groupedCount: groupedMessages.length,
      streamingToolMessages: streamingToolMessageIds,
    });
  }

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0 overflow-hidden">
      <ScrollArea
        viewportRef={scrollContainerRef}
        className="flex-1"
        viewportProps={{
          className: 'h-full w-full',
        }}
      >
        <div
          ref={contentRef}
          className="p-4 pb-32 flex flex-col gap-6 min-h-full"
        >
          {(hasOlderMessages || isLoadingOlderMessages) && (
            <div className="flex justify-center">
              <div className="rounded-full border border-border/60 bg-background/80 px-3 py-1 text-xs text-muted-foreground shadow-sm">
                {isLoadingOlderMessages
                  ? 'Loading older messages...'
                  : 'Scroll up to load older messages'}
              </div>
            </div>
          )}
          {groupedMessages.map((groupedMessage) => {
            const isCompactBoundary = groupedMessageContainsBoundary(
              groupedMessage,
              compactedRange?.toId,
            );

            if (groupedMessage.type === 'tool_group') {
              return (
                <React.Fragment key={groupedMessage.message.id}>
                  <AgentMessageBubble
                    message={groupedMessage.message}
                    assistantName={assistantName}
                    toolResultsMap={toolResultsMap}
                    groupedToolCalls={groupedMessage.toolGroup.calls}
                    groupedMessages={groupedMessage.messages}
                    isPending={pendingMessageIds.has(groupedMessage.message.id)}
                  />
                  {isCompactBoundary && (
                    <CompactEventDivider
                      key={`compact-divider-${groupedMessage.message.id}`}
                      earlierPreview={compactedEvent?.earlierPreview}
                      latestIncludedPreview={
                        compactedEvent?.latestIncludedPreview
                      }
                      condensedCount={compactedEvent?.condensedCount}
                      summary={compactedEvent?.summary}
                    />
                  )}
                </React.Fragment>
              );
            }

            if (groupedMessage.type === 'tool_error_group') {
              return (
                <React.Fragment key={groupedMessage.message.id}>
                  <AgentMessageBubble
                    message={groupedMessage.message}
                    assistantName={assistantName}
                    groupedMessages={groupedMessage.messages}
                    isPending={pendingMessageIds.has(groupedMessage.message.id)}
                    toolErrorGroup={true}
                  />
                  {isCompactBoundary && (
                    <CompactEventDivider
                      key={`compact-divider-${groupedMessage.message.id}`}
                      earlierPreview={compactedEvent?.earlierPreview}
                      latestIncludedPreview={
                        compactedEvent?.latestIncludedPreview
                      }
                      condensedCount={compactedEvent?.condensedCount}
                      summary={compactedEvent?.summary}
                    />
                  )}
                </React.Fragment>
              );
            }

            // Handle message-level errors
            if (groupedMessage.message.error) {
              return (
                <React.Fragment key={groupedMessage.message.id}>
                  <div className="self-start my-2">
                    <ErrorBubble
                      error={groupedMessage.message.error}
                      onRetry={retryMessage}
                    />
                  </div>
                  {isCompactBoundary && (
                    <CompactEventDivider
                      key={`compact-divider-${groupedMessage.message.id}`}
                      earlierPreview={compactedEvent?.earlierPreview}
                      latestIncludedPreview={
                        compactedEvent?.latestIncludedPreview
                      }
                      condensedCount={compactedEvent?.condensedCount}
                      summary={compactedEvent?.summary}
                    />
                  )}
                </React.Fragment>
              );
            }

            // Render regular message
            const msg = groupedMessage.message;

            // Check if message has any renderable content
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
              return null;
            }

            return (
              <React.Fragment key={msg.id}>
                <AgentMessageBubble
                  message={msg}
                  assistantName={assistantName}
                  isPending={pendingMessageIds.has(msg.id)}
                />
                {isCompactBoundary && (
                  <CompactEventDivider
                    key={`compact-divider-${msg.id}`}
                    earlierPreview={compactedEvent?.earlierPreview}
                    latestIncludedPreview={
                      compactedEvent?.latestIncludedPreview
                    }
                    condensedCount={compactedEvent?.condensedCount}
                    summary={compactedEvent?.summary}
                  />
                )}
              </React.Fragment>
            );
          })}

          {/* Global (top-level) error: render aligned with assistant bubbles */}
          {agentError && (
            <div className="self-start mt-2">
              <ErrorBubble error={agentError} onRetry={retryMessage} />
            </div>
          )}

          {/* LLM specific error (e.g. malformed function call) */}
          {agentLlmError && (
            <div className="self-start mt-2">
              <ErrorBubble error={agentLlmError} onRetry={retryMessage} />
            </div>
          )}
          {/* Global/Bottom AnalysisLoader: Show when busy but nothing is streaming/meaningful yet */}
          {workflowStatus === 'busy' &&
            (lastMessageWho !== 'assistant' ||
              (messages[messages.length - 1]?.role === 'assistant' &&
                !messages[messages.length - 1]?.content?.length &&
                !messages[messages.length - 1]?.thinking &&
                !messages[messages.length - 1]?.tool_calls?.length)) && (
              <div className="flex justify-start mb-8 mt-3">
                <div className="w-full max-w-full bg-secondary/30 rounded-lg px-6 py-5">
                  <div className="flex items-center gap-3 mb-2">
                    <div className="w-7 h-7 bg-primary rounded-full flex items-center justify-center animate-pulse">
                      <Bot size={16} className="text-primary-foreground" />
                    </div>
                    <span className="text-xs font-medium">
                      {session?.assistant?.name || 'Agent'}
                    </span>
                  </div>
                  <div className="text-sm">
                    <AnalysisLoader size="md" />
                  </div>
                </div>
              </div>
            )}

          {/* Pending Approvals */}
          {pendingApprovals && pendingApprovals.length > 0 && (
            <div className="flex justify-start mb-8 mt-3">
              <PendingApprovalWidget
                approvals={pendingApprovals}
                onRespond={respondToToolApproval}
              />
            </div>
          )}

          <div ref={messagesEndRef} />
        </div>
      </ScrollArea>
    </div>
  );
}
