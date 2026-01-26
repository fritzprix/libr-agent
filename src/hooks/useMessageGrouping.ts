import { useMemo, useRef } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { MCPContent } from '@/lib/mcp';

export type GroupedMessage =
  | { type: 'single'; message: Message }
  | {
      type: 'tool_group';
      message: Message;
      messages: Message[]; // All messages in this group
      toolGroup: {
        calls: ToolCall[];
        results: (Message | undefined)[];
      };
    };

export interface MessageGroupingResult {
  groupedMessages: GroupedMessage[];
  toolResultsMap: Map<string, Message>;
}

function validText(c: MCPContent) {
  if (c.type === 'text') {
    return c.text && c.text.trim().length > 0;
  } else if (c.type === 'thinking') {
    return c.thinking && c.thinking.trim().length > 0;
  }
}

interface GroupCacheEntry {
  inputs: Message[];
  output: GroupedMessage;
}

export function useMessageGrouping(messages: Message[]): MessageGroupingResult {
  const groupCache = useRef<Map<string, GroupCacheEntry>>(new Map());

  return useMemo(() => {
    const groupedMessages: GroupedMessage[] = [];
    const toolResultsMap = new Map<string, Message>();
    const nextCache = new Map<string, GroupCacheEntry>();
    const currentCache = groupCache.current;

    // Helper: Check if message has text content
    const hasTextContent = (msg: Message): boolean => {
      // Check for thinking content
      if (msg.thinking && msg.thinking.trim().length > 0) {
        return true;
      }

      return (
        !!msg.content && msg.content.length > 0 && msg.content.some(validText)
      );
    };

    // Helper: shallow compare arrays
    const areMessagesEqual = (a: Message[], b: Message[]) => {
      if (a.length !== b.length) return false;
      for (let k = 0; k < a.length; k++) {
        if (a[k] !== b[k]) return false;
      }
      return true;
    };

    let i = 0;
    while (i < messages.length) {
      const msg = messages[i];
      const startIndex = i;

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
        const groupMessages: Message[] = []; // Collect all messages in the group
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

          groupMessages.push(currentMsg);
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

        // Determine inputs for cache check
        // The messages involved are from startIndex to j (exclusive)
        const inputs = messages.slice(startIndex, j);

        // Group if there are any tool calls
        if (allToolCalls.length > 0) {
          // Check cache
          const cached = currentCache.get(msg.id);
          if (cached && areMessagesEqual(cached.inputs, inputs)) {
            groupedMessages.push(cached.output);
            nextCache.set(msg.id, cached);
          } else {
            // Pre-calculate results array to avoid O(K) mapping in render loop
            const results = allToolCalls.map((call) =>
              toolResultsMap.get(call.id),
            );

            const output: GroupedMessage = {
              type: 'tool_group',
              message: msg,
              messages: groupMessages,
              toolGroup: { calls: allToolCalls, results },
            };
            groupedMessages.push(output);
            nextCache.set(msg.id, { inputs, output });
          }
        } else {
          // Fallback (shouldn't really happen due to outer if, but safe)
          const inputs = [msg];
          const cached = currentCache.get(msg.id);
          if (cached && areMessagesEqual(cached.inputs, inputs)) {
            groupedMessages.push(cached.output);
            nextCache.set(msg.id, cached);
          } else {
            const output: GroupedMessage = { type: 'single', message: msg };
            groupedMessages.push(output);
            nextCache.set(msg.id, { inputs, output });
          }
        }
        i = j;
      } else {
        // Regular message (user or assistant without tool calls)
        const inputs = [msg];
        const cached = currentCache.get(msg.id);
        if (cached && areMessagesEqual(cached.inputs, inputs)) {
          groupedMessages.push(cached.output);
          nextCache.set(msg.id, cached);
        } else {
          const output: GroupedMessage = { type: 'single', message: msg };
          groupedMessages.push(output);
          nextCache.set(msg.id, { inputs, output });
        }
        i++;
      }
    }

    // Update cache ref for next render
    groupCache.current = nextCache;

    return { groupedMessages, toolResultsMap };
  }, [messages]);
}
