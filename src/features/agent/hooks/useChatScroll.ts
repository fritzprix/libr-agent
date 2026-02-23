import { useEffect, useRef, useState } from 'react';
import { useThrottle } from '@/hooks/useThrottle';
import type { Message } from '@/models/chat';

interface UseChatScrollProps {
  messages: Message[];
}

export function useChatScroll({ messages }: UseChatScrollProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);

  // Keep track of previous message count to determine scroll behavior
  const prevMessagesLength = useRef(messages.length);

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

  return {
    messagesEndRef,
    scrollContainerRef,
    autoScrollEnabled,
  };
}
