import { useCallback, useEffect, useRef, useState } from 'react';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useMessageGrouping } from '@/hooks/useMessageGrouping';
import { useThrottle } from '@/hooks/useThrottle';
import { AgentToolCallGroup } from './AgentToolCallGroup';
import { AgentMessageRenderer } from './AgentMessageRenderer';
import { ErrorBubble } from '@/features/chat/ErrorBubble';
import { Bot } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';

export function AgentChatMessages() {
  const { messages, isLoading, error, retryMessage } = useAgentChat();
  const { currentSession } = useAgentSessionState();

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);

  // Group messages for display
  const groupedMessages = useMessageGrouping(messages);

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

  // Get assistant name for message (Agent V2 uses generic "Agent" label)
  const getAssistantNameForMessage = useCallback((msg: Message) => {
    if (msg.role === 'assistant') {
      return 'Agent';
    }
    return '';
  }, []);

  // Adapter to satisfy ErrorBubble's onRetry signature
  const handleRetry = async () => {
    return retryMessage();
  };

  return (
    <div className="flex-1 flex flex-col min-h-0 min-w-0">
      <div
        ref={scrollContainerRef}
        className="flex-1 p-4 overflow-y-auto overflow-x-hidden flex flex-col gap-6 terminal-scrollbar"
      >
        {groupedMessages.map((groupedMessage, index) => {
          if (groupedMessage.type === 'tool_group') {
            return (
              <AgentToolCallGroup
                key={groupedMessage.message.id}
                message={groupedMessage.message}
                toolGroup={groupedMessage.toolGroup}
                isLast={index === groupedMessages.length - 1}
              />
            );
          }

          // Handle message-level errors
          if (groupedMessage.message.error) {
            return (
              <div
                className="self-start mt-2 mb-2"
                key={groupedMessage.message.id}
              >
                <ErrorBubble
                  error={groupedMessage.message.error}
                  onRetry={handleRetry}
                />
              </div>
            );
          }

          // Render regular message
          const msg = groupedMessage.message;
          return (
            <div key={msg.id} className="px-4 py-2">
              <div
                className={cn(
                  'flex',
                  msg.role === 'user' ? 'justify-end' : 'justify-start',
                )}
              >
                <div
                  className={cn(
                    'inline-block max-w-[70%] p-3 rounded-lg',
                    msg.role === 'user'
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-secondary text-secondary-foreground',
                  )}
                >
                  <div className="text-xs font-semibold mb-1 opacity-70">
                    {msg.role === 'assistant'
                      ? getAssistantNameForMessage(msg) || 'ASSISTANT'
                      : msg.role.toUpperCase()}
                  </div>
                  <div className="whitespace-pre-wrap">
                    {(msg.content && msg.content.length > 0) || msg.thinking ? (
                      <>
                        {/* Thinking bubble (shown during reasoning phase) */}
                        {msg.thinking && (
                          <div className="flex flex-col gap-2 mb-3 p-3 bg-popover rounded-lg border border-border">
                            <div className="flex items-center gap-2 text-xs font-medium opacity-70">
                              {msg.isStreaming && (
                                <span className="flex gap-1">
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
                                </span>
                              )}
                              <span>Thinking Process</span>
                            </div>
                            <div className="text-xs opacity-50 italic whitespace-pre-wrap max-h-32 overflow-y-auto">
                              {msg.thinking}
                            </div>
                          </div>
                        )}

                        {/* Actual content (shown after thinking completes) */}
                        {msg.content && msg.content.length > 0 && (
                          <AgentMessageRenderer
                            content={msg.content}
                            message={msg}
                          />
                        )}
                      </>
                    ) : (
                      <span className="text-muted-foreground italic">
                        No content
                      </span>
                    )}
                  </div>
                </div>
              </div>
            </div>
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

        {isLoading && (
          <div className="flex justify-start mb-8 mt-3">
            <div className="w-full max-w-full bg-secondary/30 rounded-lg px-6 py-5">
              <div className="flex items-center gap-3 mb-2">
                <div className="w-7 h-7 bg-primary rounded-full flex items-center justify-center animate-pulse">
                  <Bot size={16} className="text-primary-foreground" />
                </div>
                <span className="text-xs font-medium">
                  Agent {currentSession?.name && `(${currentSession.name})`}
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
