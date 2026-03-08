import { useMemo } from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
import { useLLMService } from '@/context/LLMServiceContext';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { useChatScroll } from '@/features/agent/hooks/useChatScroll';
import { useFileRefetcher } from '@/features/agent/hooks/useFileRefetcher';
import { useMessageGrouping } from '@/hooks/useMessageGrouping';
import { AgentMessageBubble } from './AgentMessageBubble';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from './shared';
import { CompactEventDivider } from './shared/CompactEventDivider';
import { Bot } from 'lucide-react';
import type { Message } from '@/models/chat';
import { PendingApprovalWidget } from './PendingApprovalWidget';

export function AgentChatMessages() {
  const {
    messages,
    pendingMessages,
    error,
    llmError,
    retryMessage,
    workflowStatus,
  } = useAgentChat();
  const { session, pendingApprovals, respondToToolApproval } =
    useAgentSession();
  const { getCompactedRange } = useLLMService();

  // Compact range for divider rendering (null if no compaction has occurred)
  const compactedRange = session?.id
    ? getCompactedRange(session.id)
    : undefined;
  const { refetchSessionFiles } = useAgentResourceAttachment();

  // Use custom hooks for side effects
  const { messagesEndRef, scrollContainerRef } = useChatScroll({ messages });
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

  // Memoize error objects so ErrorBubble memo stays effective during streaming re-renders
  const agentError = useMemo(
    () =>
      error
        ? ({
            type: 'AI_SERVICE_ERROR' as const,
            displayMessage: error,
            recoverable: true,
          } satisfies NonNullable<Message['error']>)
        : null,
    [error],
  );

  const agentLlmError = useMemo(
    () =>
      llmError
        ? ({
            type: 'MALFORMED_FUNCTION_CALL' as const,
            displayMessage: llmError,
            recoverable: true,
          } satisfies NonNullable<Message['error']>)
        : null,
    [llmError],
  );

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
                assistantName={assistantName}
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
                assistantName={assistantName}
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
                  onRetry={retryMessage}
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

          const isCompactBoundary = compactedRange?.toId === msg.id;

          return (
            <>
              <AgentMessageBubble
                key={msg.id}
                message={msg}
                assistantName={assistantName}
                isPending={pendingMessageIds.has(msg.id)}
              />
              {isCompactBoundary && (
                <CompactEventDivider key={`compact-divider-${msg.id}`} />
              )}
            </>
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
    </div>
  );
}
