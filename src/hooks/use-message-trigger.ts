import { useEffect, useRef } from 'react';
import { useChatState } from '@/context/ChatContext';
import type { Message } from '@/models/chat';

interface UseMessageTriggerOptions {
  enabled?: boolean;
  debounceMs?: number;
  messageFilter?: (message: Message) => boolean;
}

export function useMessageTrigger(
  callback: () => void | Promise<void>,
  options: UseMessageTriggerOptions = {},
) {
  const { messages } = useChatState();
  const lastHandledRef = useRef<{ id?: string }>({});
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    const { enabled = true, debounceMs = 0, messageFilter } = options;

    if (!enabled || messages.length === 0) return;

    const lastMessage = messages[messages.length - 1];

    // Apply message filter if provided
    if (messageFilter && !messageFilter(lastMessage)) {
      return;
    }

    // Prevent duplicate processing
    if (lastHandledRef.current.id === lastMessage.id) {
      return;
    }

    lastHandledRef.current.id = lastMessage.id;

    // Clear existing timeout
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current);
    }

    // Apply debouncing
    timeoutRef.current = setTimeout(() => {
      callback();
    }, debounceMs);

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, [messages, callback, options]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);
}
