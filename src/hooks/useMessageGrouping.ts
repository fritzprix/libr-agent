import { useMemo } from 'react';
import type { Message, ToolCall } from '@/models/chat';

export type GroupedMessage =
  | { type: 'single'; message: Message }
  | {
      type: 'tool_group';
      message: Message;
      toolGroup: { calls: ToolCall[] };
    };

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
 */
export function useMessageGrouping(messages: Message[]): GroupedMessage[] {
  return useMemo(() => {
    const result: GroupedMessage[] = [];

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
            j++;
          }
        }

        // Group if there are any tool calls
        if (allToolCalls.length > 0) {
          result.push({
            type: 'tool_group',
            message: msg,
            toolGroup: { calls: allToolCalls },
          });
        } else {
          // Fallback (shouldn't really happen due to outer if, but safe)
          result.push({ type: 'single', message: msg });
        }
        i = j;
      } else {
        // Regular message (user or assistant without tool calls)
        result.push({ type: 'single', message: msg });
        i++;
      }
    }

    return result;
  }, [messages]);
}
