import { useCallback, useEffect, useRef, useState } from 'react';
import { useChatState } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';
import MessageBubble from '../MessageBubble';
import { Message } from '@/models/chat';

export function ChatMessages() {
  const { messages, isLoading } = useChatState();
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
        {isLoading && (
          <div className="flex justify-start">
            <div className="rounded px-3 py-2">
              <div className="text-xs mb-1">
                Agent ({currentSession?.assistants[0]?.name})
              </div>
              <div className="text-sm">thinking...</div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}
