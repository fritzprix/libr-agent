import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useChatState, useChatActions } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';
import MessageBubble from '../MessageBubble';
import { Message, ToolCall } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';
import { getLogger } from '@/lib/logger';
import { useThrottle } from '@/hooks/useThrottle';
import { Bot } from 'lucide-react';
import { ToolCallGroupBubble } from '../ToolCallGroupBubble';

const logger = getLogger('ChatMessages');

interface GroupedMessage {
  type: 'single' | 'tool_group';
  message: Message;
  nextMessages: Message[];
  toolGroup?: {
    calls: Array<{
      toolCall: ToolCall;
      toolResult?: Message;
    }>;
  };
}

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

    logger.info('=== Starting message grouping ===', {
      totalMessages: messages.length,
    });

    let i = 0;
    while (i < messages.length) {
      const msg = messages[i];

      if (
        msg.role === 'assistant' &&
        msg.tool_calls &&
        msg.tool_calls.length > 0
      ) {
        // Collect consecutive assistant messages with tool calls
        // Start from current message (including multipart)
        const consecutiveToolMessages: Message[] = [];
        let j = i;
        let shouldStopGrouping = false;

        while (j < messages.length && !shouldStopGrouping) {
          const currentMsg = messages[j];

          // Check if this is an assistant message with tool calls
          if (
            currentMsg.role !== 'assistant' ||
            !currentMsg.tool_calls ||
            currentMsg.tool_calls.length === 0
          ) {
            break;
          }

          const currentHasText =
            currentMsg.content &&
            currentMsg.content.length > 0 &&
            currentMsg.content.some(
              (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
            );

          const currentHasUIResource =
            currentMsg.content &&
            currentMsg.content.length > 0 &&
            currentMsg.content.some(
              (c) =>
                c.type === 'resource' &&
                'mimeType' in c &&
                (c.mimeType === 'text/html' ||
                  c.mimeType === 'application/vnd.mcp-ui.remote-dom'),
            );

          // If this is NOT the first message and has text (multipart)
          // Stop here - it starts a NEW group
          if (currentHasText && j > i) {
            logger.debug('Hit multipart message - starts new group', {
              currentMessageId: currentMsg.id,
            });
            break;
          }

          // Add current message to group
          consecutiveToolMessages.push(currentMsg);

          // If first message has text, log it as group starter
          if (currentHasText && j === i) {
            logger.debug('Multipart message starting new group', {
              messageId: currentMsg.id,
            });
          }

          // If this message has UI resource, include it but stop grouping
          if (currentHasUIResource) {
            logger.debug('Message with UI resource - ending group', {
              messageId: currentMsg.id,
            });
            shouldStopGrouping = true;
          }

          j++;

          // Skip tool result messages for this assistant message
          const toolCallIds = new Set(currentMsg.tool_calls.map((tc) => tc.id));
          while (
            j < messages.length &&
            messages[j].role === 'tool' &&
            messages[j].tool_call_id &&
            toolCallIds.has(messages[j].tool_call_id!)
          ) {
            j++;
          }

          // If we're stopping due to UI resource, break now
          if (shouldStopGrouping) {
            break;
          }
        }

        // Collect ALL tool results for consecutive messages
        const allToolCalls: ToolCall[] = [];
        const allToolResults: Message[] = [];

        consecutiveToolMessages.forEach((assistantMsg) => {
          if (assistantMsg.tool_calls) {
            allToolCalls.push(...assistantMsg.tool_calls);

            // Find tool results for this assistant message
            const toolCallIds = new Set(
              assistantMsg.tool_calls.map((tc) => tc.id),
            );
            messages.forEach((m) => {
              if (
                m.role === 'tool' &&
                m.tool_call_id &&
                toolCallIds.has(m.tool_call_id)
              ) {
                allToolResults.push(m);
              }
            });
          }
        });

        // Check if first message has text (multipart group starter)
        const firstMessageHasText =
          msg.content &&
          msg.content.length > 0 &&
          msg.content.some(
            (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
          );

        // Should group if: >= 2 consecutive messages OR >= 2 tool calls in total
        // Always group multipart messages with following tool-only messages
        const shouldGroup =
          consecutiveToolMessages.length >= 2 || allToolCalls.length >= 2;

        logger.debug('Grouping decision for consecutive messages', {
          startMessageId: msg.id,
          consecutiveCount: consecutiveToolMessages.length,
          totalToolCalls: allToolCalls.length,
          firstMessageHasText,
          shouldGroup,
        });

        if (shouldGroup) {
          // Create a tool group from consecutive messages
          logger.info('Creating tool group from consecutive messages', {
            startMessageId: msg.id,
            messageCount: consecutiveToolMessages.length,
            toolCallCount: allToolCalls.length,
            startsWithMultipart: firstMessageHasText,
          });

          result.push({
            type: 'tool_group',
            message: msg,
            nextMessages: [],
            toolGroup: {
              calls: allToolCalls.map((tc) => ({
                toolCall: tc,
                toolResult: allToolResults.find(
                  (r) => r.tool_call_id === tc.id,
                ),
              })),
            },
          });

          i = j; // Skip all processed messages
        } else {
          // Single message with tool calls (not enough to group)
          const toolCallIds = new Set(msg.tool_calls.map((tc) => tc.id));
          const nextToolResults: Message[] = [];

          let k = i + 1;
          while (
            k < messages.length &&
            nextToolResults.length < toolCallIds.size
          ) {
            const nextMsg = messages[k];
            if (
              nextMsg.role === 'tool' &&
              nextMsg.tool_call_id &&
              toolCallIds.has(nextMsg.tool_call_id)
            ) {
              nextToolResults.push(nextMsg);
              k++;
            } else {
              break;
            }
          }

          logger.debug('Keeping as single bubble', {
            messageId: msg.id,
            reason: 'not enough consecutive messages to group',
          });

          result.push({
            type: 'single',
            message: msg,
            nextMessages: nextToolResults,
          });

          i = k;
        }
      } else if (msg.role !== 'tool') {
        // Regular message (not a standalone tool result)
        result.push({
          type: 'single',
          message: msg,
          nextMessages: [],
        });
        i++;
      } else {
        // Standalone tool result (no matching call) - skip it
        i++;
      }
    }

    return result;
  }, [messages]);

  // Debug: Log grouped messages structure
  useEffect(() => {
    logger.info('Grouped messages updated', {
      total: groupedMessages.length,
      groups: groupedMessages.filter((g) => g.type === 'tool_group').length,
      singles: groupedMessages.filter((g) => g.type === 'single').length,
      details: groupedMessages.map((g) => ({
        type: g.type,
        id: g.message.id,
        toolCallCount: g.message.tool_calls?.length || 0,
        hasToolGroup: !!g.toolGroup,
      })),
    });
  }, [groupedMessages]);

  const { retryMessage } = useChatActions();
  // Adapter to satisfy ErrorBubble's onRetry signature which may pass undefined
  const handleRetry = async () => {
    return retryMessage();
  };

  logger.info('error : ', { error });

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div
        ref={scrollContainerRef}
        className="flex-1 p-4 overflow-y-auto flex flex-col gap-6 terminal-scrollbar"
      >
        {groupedMessages.map(({ type, message, nextMessages, toolGroup }) => {
          logger.debug('Rendering message', {
            type,
            id: message.id,
            hasToolGroup: !!toolGroup,
            toolGroupCallCount: toolGroup?.calls.length,
          });

          return type === 'tool_group' && toolGroup ? (
            <ToolCallGroupBubble
              key={message.id}
              message={message}
              toolGroup={toolGroup}
            />
          ) : (
            <MessageBubble
              key={message.id}
              message={message}
              nextMessages={nextMessages}
              currentAssistantName={getAssistantNameForMessage(message)}
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
