import { useEffect, useRef } from 'react';
import { useAgentChatState } from '@/context/AgentChatContext';
import type { Message } from '@/models/chat';

interface UseAgentMessageTriggerOptions {
  enabled?: boolean;
  debounceMs?: number;
  messageFilter?: (message: Message) => boolean;
}

/**
 * Agent V2 message trigger hook
 *
 * Similar to Chat V1's useMessageTrigger but for Agent V2 context.
 * Triggers callback when messages array changes (new message arrives).
 *
 * Use cases:
 * - Update service contexts when assistant messages arrive
 * - Refresh panel states after tool execution
 * - Trigger side effects on message completion
 */
export function useAgentMessageTrigger(
  callback: () => void | Promise<void>,
  options: UseAgentMessageTriggerOptions = {},
) {
  const { messages } = useAgentChatState();
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
