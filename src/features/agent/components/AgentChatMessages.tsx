import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useMessageGrouping } from '@/hooks/useMessageGrouping';
import { useThrottle } from '@/hooks/useThrottle';
import { AgentMessageBubble } from './AgentMessageBubble';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from './shared';
import { Bot } from 'lucide-react';
import type { Message } from '@/models/chat';

export function AgentChatMessages() {
  const {
    messages,
    pendingMessages,
    error,
    llmError,
    retryMessage,
    workflowStatus,
  } = useAgentChat();
  const { session } = useAgentSessionState();
  const { refetchSessionFiles } = useAgentResourceAttachment();

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);
  // Keep track of previous message count to determine scroll behavior
  const prevMessagesLength = useRef(messages.length);

  // Group messages for display
  const { groupedMessages, toolResultsMap } = useMessageGrouping(messages);

  // Convert pendingMessages to a Set of IDs for O(1) lookups
  // This prevents O(n*m) performance issues when checking if each message is pending
  const pendingMessageIds = useMemo(
    () => new Set(pendingMessages.map((msg) => msg.id)),
    [pendingMessages],
  );

  // Only auto-scroll if enabled
  useEffect(() => {
    if (autoScrollEnabled) {
      // If we have a NEW message, smooth scroll.
      // If we are just streaming (same message count), jump to bottom (auto) to avoid jank.
      const isNewMessage = messages.length > prevMessagesLength.current;
      const behavior = isNewMessage ? 'smooth' : 'auto';

      messagesEndRef.current?.scrollIntoView({ behavior });
    }
    prevMessagesLength.current = messages.length;
  }, [messages, autoScrollEnabled]);

  // Throttle the refetch function to prevent excessive backend calls
  const throttledRefetch = useThrottle(() => {
    refetchSessionFiles();
  }, 2000);

  // Refetch session files when message stack updates
  // This ensures SessionFilesPopover reflects any files added by agent tool calls
  useEffect(() => {
    if (messages.length > 0) {
      // Check if last message contains tool results (file operations)
      const lastMessage = messages[messages.length - 1];
      // Only refetch when we have a tool result (role === 'tool').
      // We do NOT refetch on 'assistant' messages with tool_calls, as the files
      // are only created after the tool execution is complete.
      if (lastMessage.role === 'tool') {
        throttledRefetch();
      }
    }
  }, [messages, throttledRefetch]);

  // Detect user scroll position with throttling to improve performance
  const handleScroll = useThrottle(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    // If user is at the bottom, enable auto-scroll
    const { scrollTop, scrollHeight, clientHeight } = container;
    const atBottom = scrollHeight - scrollTop - clientHeight < 10;
    setAutoScrollEnabled(atBottom);
  }, 100);

  const lastMessageWho = messages[messages.length - 1]?.role;

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const getAssistantNameForMessage = useCallback(
    (msg: Message) => {
      if (msg?.role === 'assistant') {
        return session?.assistant?.name || 'Agent';
      }
      return '';
    },
    [session?.assistant?.name],
  );

  // Adapter to satisfy ErrorBubble's onRetry signature
  const handleRetry = async () => {
    return retryMessage();
  };

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0">
      <div
        ref={scrollContainerRef}
        className="flex-1 p-4 overflow-y-auto overflow-x-hidden flex flex-col gap-6"
      >
        {groupedMessages.map((groupedMessage) => {
          if (groupedMessage.type === 'tool_group') {
            return (
              <AgentMessageBubble
                key={groupedMessage.message.id}
                message={groupedMessage.message}
                getAssistantName={getAssistantNameForMessage}
                toolResultsMap={toolResultsMap}
                groupedToolCalls={groupedMessage.toolGroup.calls}
                groupedMessages={groupedMessage.messages}
                isPending={pendingMessageIds.has(groupedMessage.message.id)}
              />
            );
          }

          if (groupedMessage.type === 'tool_error_group') {
            return (
              <AgentMessageBubble
                key={groupedMessage.message.id}
                message={groupedMessage.message}
                getAssistantName={getAssistantNameForMessage}
                groupedMessages={groupedMessage.messages}
                isPending={pendingMessageIds.has(groupedMessage.message.id)}
                toolErrorGroup={true}
              />
            );
          }

          // Handle message-level errors
          if (groupedMessage.message.error) {
            return (
              <div className="self-start my-2" key={groupedMessage.message.id}>
                <ErrorBubble
                  error={groupedMessage.message.error}
                  onRetry={handleRetry}
                />
              </div>
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
            <AgentMessageBubble
              key={msg.id}
              message={msg}
              getAssistantName={getAssistantNameForMessage}
              isPending={pendingMessageIds.has(msg.id)}
            />
          );
        })}

        {/* Global (top-level) error: render aligned with assistant bubbles */}
        {error && (
          <div className="self-start mt-2">
            <ErrorBubble
              error={{
                type: 'AI_SERVICE_ERROR',
                displayMessage: error,
                recoverable: true,
              }}
              onRetry={handleRetry}
            />
          </div>
        )}

        {/* LLM specific error (e.g. malformed function call) */}
        {llmError && (
          <div className="self-start mt-2">
            <ErrorBubble
              error={{
                type: 'MALFORMED_FUNCTION_CALL',
                displayMessage: llmError,
                recoverable: true,
              }}
              onRetry={handleRetry}
            />
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

        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}
