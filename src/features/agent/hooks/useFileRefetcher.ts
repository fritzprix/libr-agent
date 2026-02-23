import { useEffect } from 'react';
import { useThrottle } from '@/hooks/useThrottle';
import type { Message } from '@/models/chat';

interface UseFileRefetcherProps {
  messages: Message[];
  refetchSessionFiles: () => Promise<void>;
  throttleMs?: number;
}

export function useFileRefetcher({
  messages,
  refetchSessionFiles,
  throttleMs = 2000,
}: UseFileRefetcherProps) {
  // Throttle the refetch function to prevent excessive backend calls
  const throttledRefetch = useThrottle(() => {
    refetchSessionFiles();
  }, throttleMs);

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
}
