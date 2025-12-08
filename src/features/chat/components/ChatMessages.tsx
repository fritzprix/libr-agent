import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useChatState, useChatActions } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';
import MessageBubble from '../MessageBubble';
import { Message, ToolCall } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';
import { useThrottle } from '@/hooks/useThrottle';
import { Bot } from 'lucide-react';
import { ToolCallGroupBubble } from '../ToolCallGroupBubble';

type GroupedMessage =
  | {
      type: 'single';
      message: Message;
    }
  | {
      type: 'tool_group';
      message: Message;
      toolGroup: {
        calls: ToolCall[];
      };
    };

export function ChatMessages() {
  const { messages, isLoading, error } = useChatState();
  const { getCurrentSession, current: currentSession } = useSessionContext();
  const { getById } = useAssistantContext();

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);

  // Only auto-scroll if enabled
  useEffect(() => {
    if (autoScrollEnabled) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, autoScrollEnabled]);

  // Detect user scroll position with throttling to improve performance
  const handleScroll = useThrottle(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    // If user is at the bottom, enable auto-scroll
    const { scrollTop, scrollHeight, clientHeight } = container;
    const atBottom = scrollHeight - scrollTop - clientHeight < 10;
    setAutoScrollEnabled(atBottom);
  }, 100);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    container.addEventListener('scroll', handleScroll, { passive: true });
    return () => container.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  const getAssistantNameForMessage = useCallback(
    (m: Message) => {
      if (m.role === 'assistant' && 'assistantId' in m && m.assistantId) {
        const assistant = getById(m.assistantId);
        return assistant?.name || '';
      }
      const currentSession = getCurrentSession();
      if (m.role === 'assistant' && currentSession?.assistants?.length) {
        return currentSession.assistants[0].name;
      }
      return '';
    },
    [getById, getCurrentSession],
  );

  // Group messages: Assistant messages with tool_calls are paired with their tool results
  // CONSECUTIVE tool-only assistant messages (even with 1 call each) are grouped into ToolCallGroupBubble
  const groupedMessages = useMemo(() => {
    const result: GroupedMessage[] = [];

    // Helper: Skip tool result messages for given tool call IDs
    const skipToolResults = (
      startIdx: number,
      toolCallIds: Set<string>,
    ): number => {
      let idx = startIdx;
      while (
        idx < messages.length &&
        messages[idx].role === 'tool' &&
        messages[idx].tool_call_id &&
        toolCallIds.has(messages[idx].tool_call_id!)
      ) {
        idx++;
      }
      return idx;
    };

    // Helper: Check if message has text content
    const hasTextContent = (msg: Message): boolean => {
      return (
        msg.content &&
        msg.content.length > 0 &&
        msg.content.some(
          (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
        )
      );
    };

    let i = 0;
    while (i < messages.length) {
      const msg = messages[i];

      // Skip standalone tool results (no matching assistant message)
      if (msg.role === 'tool') {
        i++;
        continue;
      }

      // Handle assistant messages with tool calls
      if (
        msg.role === 'assistant' &&
        msg.tool_calls &&
        msg.tool_calls.length > 0
      ) {
        const allToolCalls: ToolCall[] = [];
        let j = i;

        // Collect consecutive assistant messages with tool calls
        while (j < messages.length) {
          const currentMsg = messages[j];

          // Stop if not an assistant message with tool calls
          if (
            currentMsg.role !== 'assistant' ||
            !currentMsg.tool_calls ||
            currentMsg.tool_calls.length === 0
          ) {
            break;
          }

          // Stop if multipart message (text + tool calls) appears after first message
          if (hasTextContent(currentMsg) && j > i) {
            break;
          }

          // Collect tool calls from this message
          allToolCalls.push(...currentMsg.tool_calls);

          // Skip to next message after tool results
          const toolCallIds = new Set(currentMsg.tool_calls.map((tc) => tc.id));
          j = skipToolResults(j + 1, toolCallIds);
        }

        // Group if there are any tool calls (ensures consistent UI for single/multiple calls)
        const shouldGroup = allToolCalls.length > 0;

        if (shouldGroup) {
          result.push({
            type: 'tool_group',
            message: msg,
            toolGroup: { calls: allToolCalls },
          });
        } else {
          result.push({
            type: 'single',
            message: msg,
          });
        }

        i = j;
      } else {
        // Regular message (user, system, or assistant without tool calls)
        result.push({
          type: 'single',
          message: msg,
        });
        i++;
      }
    }

    return result;
  }, [messages]);

  const { retryMessage } = useChatActions();
  // Adapter to satisfy ErrorBubble's onRetry signature which may pass undefined
  const handleRetry = async (messageIdToDelete?: string) => {
    return retryMessage(messageIdToDelete);
  };

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div
        ref={scrollContainerRef}
        className="flex-1 p-4 overflow-y-auto overflow-x-hidden flex flex-col gap-6 terminal-scrollbar"
      >
        {groupedMessages.map((groupedMessage, index) => {
          if (groupedMessage.type === 'tool_group') {
            return (
              <ToolCallGroupBubble
                key={groupedMessage.message.id}
                message={groupedMessage.message}
                toolGroup={groupedMessage.toolGroup}
                isLast={index === groupedMessages.length - 1}
              />
            );
          }

          if (groupedMessage.message.error) {
            return (
              <div
                className="self-start mt-2 mb-2"
                key={groupedMessage.message.id}
              >
                <ErrorBubble
                  error={groupedMessage.message.error}
                  onRetry={() => handleRetry(groupedMessage.message.id)}
                />
              </div>
            );
          }

          return (
            <MessageBubble
              key={groupedMessage.message.id}
              message={groupedMessage.message}
              currentAssistantName={getAssistantNameForMessage(
                groupedMessage.message,
              )}
              isLast={index === groupedMessages.length - 1}
            />
          );
        })}
        {/* Global (top-level) assistant error: render aligned with assistant bubbles */}
        {error && (
          <div className="self-start mt-2">
            <ErrorBubble error={error} onRetry={handleRetry} />
          </div>
        )}
        {isLoading && (
          <div className="flex justify-start mb-8 mt-3">
            <div className="w-full max-w-full bg-secondary/30 rounded-lg px-6 py-5">
              <div className="flex items-center gap-3 mb-2">
                <div className="w-7 h-7 bg-primary rounded-full flex items-center justify-center animate-pulse">
                  <Bot size={16} className="text-primary-foreground" />
                </div>
                <span className="text-xs font-medium">
                  Agent ({currentSession?.assistants[0]?.name})
                </span>
              </div>
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <div className="flex gap-1">
                  <span
                    className="animate-bounce"
                    style={{ animationDelay: '0ms' }}
                  >
                    ●
                  </span>
                  <span
                    className="animate-bounce"
                    style={{ animationDelay: '150ms' }}
                  >
                    ●
                  </span>
                  <span
                    className="animate-bounce"
                    style={{ animationDelay: '300ms' }}
                  >
                    ●
                  </span>
                </div>
                <span className="animate-pulse">Thinking and analyzing...</span>
              </div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}
