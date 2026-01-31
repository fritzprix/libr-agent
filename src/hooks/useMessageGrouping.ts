import { useMemo, useRef, useEffect } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { MCPContent } from '@/lib/mcp';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useMessageGrouping');

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

// Helper: Check if message has text content
const hasTextContent = (msg: Message): boolean => {
  // Check for thinking content
  if (msg.thinking && msg.thinking.trim().length > 0) {
    return true;
  }

  return !!msg.content && msg.content.length > 0 && msg.content.some(validText);
};

// Helper: Check if two maps are identical (shallow equality of keys and values)
function areMapsEqual(
  map1: Map<string, Message>,
  map2: Map<string, Message>,
): boolean {
  if (map1.size !== map2.size) return false;
  for (const [key, val] of map1) {
    if (!map2.has(key)) return false;
    if (map2.get(key) !== val) return false;
  }
  return true;
}

export function useMessageGrouping(messages: Message[]): MessageGroupingResult {
  // Cache the previous result and metadata to enable differential updates
  const cache = useRef<{
    messages: Message[];
    groupedMessages: GroupedMessage[];
    // Track where each grouped message "ends" in the original messages array
    // This allows us to know which groups are affected by changes at index X
    groupEndIndices: number[];
    toolResultsMap: Map<string, Message>;
  }>({
    messages: [],
    groupedMessages: [],
    groupEndIndices: [],
    toolResultsMap: new Map(),
  });

  const calculation = useMemo(() => {
    const prevCache = cache.current;

    // 1. Find the divergence point (where messages differ from previous render)
    let divergenceIndex = 0;
    while (
      divergenceIndex < prevCache.messages.length &&
      divergenceIndex < messages.length &&
      prevCache.messages[divergenceIndex] === messages[divergenceIndex]
    ) {
      divergenceIndex++;
    }

    // 2. Identify stable groups
    // CRITICAL: Only reuse groups that end strictly BEFORE the divergence point.
    // If a group ends AT the divergence point (endIndex == divergenceIndex), it means the messages
    // *immediately following* the group have changed or are new.
    // Since "tool_group" messages can consume subsequent tool results, we must re-evaluate
    // the last group to see if it should now consume the new/changed messages.
    let reuseCount = 0;
    for (let k = 0; k < prevCache.groupEndIndices.length; k++) {
      if (prevCache.groupEndIndices[k] < divergenceIndex) {
        reuseCount++;
      } else {
        break;
      }
    }

    // 3. Initialize with reused data
    const groupedMessages: GroupedMessage[] = [];
    const groupEndIndices: number[] = [];
    // Always create a fresh map per calculation to avoid stale tool result entries.
    const toolResultsMap = new Map<string, Message>();

    // Reuse previously computed groups where safe.
    if (reuseCount > 0) {
      for (let k = 0; k < reuseCount; k++) {
        groupedMessages.push(prevCache.groupedMessages[k]);
        groupEndIndices.push(prevCache.groupEndIndices[k]);
      }
    }

    // 4. Process new/changed messages
    // Start index is the end index of the last reused group (or 0)
    let i = reuseCount > 0 ? groupEndIndices[reuseCount - 1] : 0;

    // Pre-populate toolResultsMap from the reused prefix of messages to keep it in sync.
    for (let prefixIndex = 0; prefixIndex < i; prefixIndex++) {
      const msg = messages[prefixIndex];
      if (msg.role === 'tool' && msg.tool_call_id) {
        toolResultsMap.set(msg.tool_call_id, msg);
      }
    }

    // Helper to capture tool results (needed for map population)
    const captureToolResult = (msg: Message) => {
      if (msg.role === 'tool' && msg.tool_call_id) {
        toolResultsMap.set(msg.tool_call_id, msg);
      }
    };

    while (i < messages.length) {
      const msg = messages[i];

      // Capture tool result in map (even if skipped later)
      if (msg.role === 'tool' && msg.tool_call_id) {
        const previous = i > 0 ? messages[i - 1] : undefined;
        const isImmediatelyAfterAssistantToolCall =
          previous?.role === 'assistant' &&
          Array.isArray(previous.tool_calls) &&
          previous.tool_calls.some((call) => call.id === msg.tool_call_id);

        if (!isImmediatelyAfterAssistantToolCall) {
          captureToolResult(msg);
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
            captureToolResult(messages[j]);
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
            messages: groupMessages,
            toolGroup: { calls: allToolCalls, results },
          });
          groupEndIndices.push(j);
        } else {
          // Defensive fallback: This theoretically shouldn't happen since the outer
          // condition checks msg.tool_calls.length > 0. If we hit this, log it and
          // treat the message as a single message.
          logger.warn(
            'Unexpected state: assistant message with tool_calls but allToolCalls is empty',
            { messageId: msg.id },
          );
          groupedMessages.push({ type: 'single', message: msg });
          groupEndIndices.push(i + 1);
          j = i + 1;
        }
        i = j;
      } else {
        // Regular message (user or assistant without tool calls)
        groupedMessages.push({ type: 'single', message: msg });
        i++;
        groupEndIndices.push(i);
      }
    }

    // Performance Optimization:
    // If the newly computed toolResultsMap is identical (by reference for keys/values)
    // to the previous one, reuse the previous Map instance.
    // This prevents AgentMessageBubble (which receives this map as a prop) from re-rendering
    // unnecessarily when tool results haven't changed (e.g. during text streaming).
    let finalToolResultsMap = toolResultsMap;
    if (areMapsEqual(toolResultsMap, prevCache.toolResultsMap)) {
      finalToolResultsMap = prevCache.toolResultsMap;
    }

    return {
      result: { groupedMessages, toolResultsMap: finalToolResultsMap },
      newCache: {
        messages,
        groupedMessages,
        groupEndIndices,
        toolResultsMap: finalToolResultsMap,
      },
    };
  }, [messages]);

  // Update cache in useEffect to keep render phase pure(r)
  useEffect(() => {
    cache.current = calculation.newCache;
  }, [calculation.newCache]);

  return calculation.result;
}
