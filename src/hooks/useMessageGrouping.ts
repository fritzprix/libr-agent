import { useMemo } from 'react';
import type { Message, ToolCall } from '@/models/chat';

export type GroupedMessage =
  | { type: 'single'; message: Message }
  | {
      type: 'tool_group';
      message: Message;
      toolGroup: {
        calls: ToolCall[];
        results: (Message | undefined)[];
      };
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
 * - Pre-calculates tool results array for each group to avoid O(K) allocation in render loops.
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
        const previous = i > 0 ? messages[i - 1] : undefined;
        const isImmediatelyAfterAssistantToolCall =
          previous?.role === 'assistant' &&
          Array.isArray(previous.tool_calls) &&
          previous.tool_calls.some((call) => call.id === msg.tool_call_id);

        // Avoid double insertion for tool results that immediately follow
        // assistant messages with matching tool_calls. Those associations
        // are handled when processing the assistant message. We still capture
        // orphan tool results here so they are available in toolResultsMap.
        if (!isImmediatelyAfterAssistantToolCall) {
          toolResultsMap.set(msg.tool_call_id, msg);
        }
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
        const groupToolCallIds = new Set<string>();
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
          currentMsg.tool_calls.forEach((tc) => groupToolCallIds.add(tc.id));

          // Skip past associated tool results
          j++;
          while (
            j < messages.length &&
            messages[j].role === 'tool' &&
            messages[j].tool_call_id &&
            groupToolCallIds.has(messages[j].tool_call_id!)
          ) {
            // Capture skipped tool result in map.
            // These tool results were already encountered when they were at position i
            // in the outer loop; we add them here as well because i will later jump to j,
            // effectively skipping these indices. The has-check avoids redundant overwrites.
            const toolCallId = messages[j].tool_call_id!;
            if (!toolResultsMap.has(toolCallId)) {
              toolResultsMap.set(toolCallId, messages[j]);
            }
            j++;
          }
        }

        // Group if there are any tool calls
        if (allToolCalls.length > 0) {
          // Pre-calculate results array to avoid O(K) mapping in render loop
          const results = allToolCalls.map((call) =>
            toolResultsMap.get(call.id),
          );

          groupedMessages.push({
            type: 'tool_group',
            message: msg,
            toolGroup: { calls: allToolCalls, results },
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
