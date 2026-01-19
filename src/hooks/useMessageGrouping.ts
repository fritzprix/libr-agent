import { useMemo } from 'react';
import type { Message, ToolCall } from '@/models/chat';

export type GroupedMessage =
  | { type: 'single'; message: Message }
  | {
      type: 'tool_group';
      message: Message;
      toolGroup: { calls: ToolCall[] };
    };

export interface MessageGroupingResult {
  groupedMessages: GroupedMessage[];
  toolResultsMap: Map<string, Message>;
}

/**
 * Groups messages for display, combining consecutive assistant messages with tool calls
 * into tool groups and leaving other messages as singles.
 *
 * Algorithm:
 * 1. Skip standalone tool results (they're displayed within tool groups)
 * 2. Group consecutive assistant messages with tool_calls
 * 3. Collect all tool calls across consecutive messages
 * 4. Skip associated tool result messages between calls
 * 5. Regular messages (user, assistant w/o tools) remain as singles
 *
 * Performance Optimization:
 * - Computes toolResultsMap in the same pass to avoid a second O(N) iteration in the consumer.
 */
export function useMessageGrouping(messages: Message[]): MessageGroupingResult {
  return useMemo(() => {
    const groupedMessages: GroupedMessage[] = [];
    const toolResultsMap = new Map<string, Message>();

    // Helper: Check if message has text content
    const hasTextContent = (msg: Message): boolean => {
      return (
        !!msg.content &&
        msg.content.length > 0 &&
        msg.content.some(
          (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
        )
      );
    };

    let i = 0;
    while (i < messages.length) {
      const msg = messages[i];

      // Capture tool result in map
      if (msg.role === 'tool' && msg.tool_call_id) {
        toolResultsMap.set(msg.tool_call_id, msg);
      }

      // Skip standalone tool results (they're shown within tool groups)
      if (msg.role === 'tool') {
        i++;
        continue;
      }

      // Group assistant messages with tool_calls
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

          allToolCalls.push(...currentMsg.tool_calls);

          // Skip past associated tool results
          const toolCallIds = new Set(currentMsg.tool_calls.map((tc) => tc.id));
          j++;
          while (
            j < messages.length &&
            messages[j].role === 'tool' &&
            messages[j].tool_call_id &&
            toolCallIds.has(messages[j].tool_call_id!)
          ) {
            // Capture skipped tool result in map
            // Note: We might have already captured it if we didn't use continue in main loop,
            // but the main loop skips if we increment i.
            // Here j is incremented.
            // We need to capture it here because these indices (j) will be skipped by i = j assignment later.
            toolResultsMap.set(messages[j].tool_call_id!, messages[j]);
            j++;
          }
        }

        // Group if there are any tool calls
        if (allToolCalls.length > 0) {
          groupedMessages.push({
            type: 'tool_group',
            message: msg,
            toolGroup: { calls: allToolCalls },
          });
        } else {
          // Fallback (shouldn't really happen due to outer if, but safe)
          groupedMessages.push({ type: 'single', message: msg });
        }
        i = j;
      } else {
        // Regular message (user or assistant without tool calls)
        groupedMessages.push({ type: 'single', message: msg });
        i++;
      }
    }

    return { groupedMessages, toolResultsMap };
  }, [messages]);
}
