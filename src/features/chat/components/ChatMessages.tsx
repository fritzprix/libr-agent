import { useCallback, useEffect, useRef, useState } from 'react';
import { useChatState, useChatActions } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';
import MessageBubble from '../MessageBubble';
import { Message } from '@/models/chat';
import { ErrorBubble } from '../ErrorBubble';
import { getLogger } from '@/lib/logger';
import { Bot } from 'lucide-react';

const logger = getLogger('ChatMessages');

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

  // Detect user scroll position
  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) return;

    const handleScroll = () => {
      // If user is at the bottom, enable auto-scroll
      const { scrollTop, scrollHeight, clientHeight } = container;
      const atBottom = scrollHeight - scrollTop - clientHeight < 10;
      setAutoScrollEnabled(atBottom);
    };

    container.addEventListener('scroll', handleScroll);
    return () => container.removeEventListener('scroll', handleScroll);
  }, []);

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
        {messages.map((m) => (
          <MessageBubble
            key={m.id}
            message={m}
            currentAssistantName={getAssistantNameForMessage(m)}
          />
        ))}
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
