import { useMemo, useRef } from 'react';
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
    }
  | {
      /**
       * A group of consecutive failed tool result messages.
       *
       * These are rendered similarly to normal tool results, but with warning/error
       * semantics for quick visual distinction.
       */
      type: 'tool_error_group';
      /** The first tool message in the error group (used as a stable key anchor). */
      message: Message;
      /** All consecutive tool error messages in the group. */
      messages: Message[];
    };

export interface MessageGroupingResult {
  groupedMessages: GroupedMessage[];
  toolResultsMap: Map<string, Message>;
}

// Optimization: Use regex to check for non-whitespace characters to avoid string allocation from trim()
const NOT_WHITESPACE = /\S/;

function validText(c: MCPContent) {
  if (c.type === 'text') {
    return c.text && NOT_WHITESPACE.test(c.text);
  } else if (c.type === 'thinking') {
    return c.thinking && NOT_WHITESPACE.test(c.thinking);
  }
}

// Helper: Check if message has text content
const hasTextContent = (msg: Message): boolean => {
  // Check for thinking content
  if (msg.thinking && NOT_WHITESPACE.test(msg.thinking)) {
    return true;
  }

  return !!msg.content && msg.content.length > 0 && msg.content.some(validText);
};

const isToolErrorMessage = (msg: Message): boolean => {
  return msg.role === 'tool' && msg.metadata?.toolError === true;
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
    // Index of the last message that contributed to the toolResultsMap.
    // If divergenceIndex > lastToolResultIndex, we can reuse the map without rebuilding.
    lastToolResultIndex: number;
  }>({
    messages: [],
    groupedMessages: [],
    groupEndIndices: [],
    toolResultsMap: new Map(),
    lastToolResultIndex: -1,
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
    // Optimization: If a 'single' group ends AT the divergence point, it is safe to reuse
    // because it cannot consume subsequent messages (unlike 'tool_group').
    // Since the next message (at divergenceIndex) has changed, we only need to ensure
    // the current group doesn't need to change its "consumption" logic.
    let reuseCount = 0;
    for (let k = 0; k < prevCache.groupEndIndices.length; k++) {
      const groupEnd = prevCache.groupEndIndices[k];
      const groupType = prevCache.groupedMessages[k].type;

      if (groupEnd < divergenceIndex) {
        reuseCount++;
      } else if (groupEnd === divergenceIndex && groupType === 'single') {
        // Safe to reuse single message groups ending exactly at divergence
        reuseCount++;
        // If we reuse a group ending at divergenceIndex, the next group starts at divergenceIndex.
        // Messages starting at divergenceIndex ARE changed, so we cannot reuse any further groups.
        break;
      } else {
        break;
      }
    }

    // 3. Initialize with reused data
    const groupedMessages: GroupedMessage[] = [];
    const groupEndIndices: number[] = [];

    // Decide whether to reuse the previous map or create a fresh one based on lastToolResultIndex
    let toolResultsMap: Map<string, Message>;
    let isMapCloned = false; // Track if we have cloned the map (copy-on-write)
    let newLastToolResultIndex = -1;

    // Reuse previously computed groups where safe.
    if (reuseCount > 0) {
      for (let k = 0; k < reuseCount; k++) {
        groupedMessages.push(prevCache.groupedMessages[k]);
        groupEndIndices.push(prevCache.groupEndIndices[k]);
      }
    }

    // 4. Initialize Tool Map
    // Optimization: If the stable prefix covers all previous tool results, reuse the map instance.
    if (divergenceIndex > prevCache.lastToolResultIndex) {
      toolResultsMap = prevCache.toolResultsMap;
      isMapCloned = false; // We are holding a ref to the old map, treated as immutable until write
      newLastToolResultIndex = prevCache.lastToolResultIndex;
    } else {
      // Rebuild map from scratch (partial prefix)
      toolResultsMap = new Map();
      isMapCloned = true; // It's a fresh map

      // We must scan the stable prefix (0..startIndex) to populate the map because we couldn't reuse the old one
      // startIndex is the end index of the last reused group (or 0 if none were reused), i.e. the start of "new/changed" processing.
      const startIndex = reuseCount > 0 ? groupEndIndices[reuseCount - 1] : 0;
      for (let prefixIndex = 0; prefixIndex < startIndex; prefixIndex++) {
        const msg = messages[prefixIndex];
        if (msg.role === 'tool' && msg.tool_call_id) {
          toolResultsMap.set(msg.tool_call_id, msg);
          newLastToolResultIndex = Math.max(
            newLastToolResultIndex,
            prefixIndex,
          );
        }
      }
    }

    // Helper to capture tool results with copy-on-write logic
    // Uses a composite key to handle LLM hallucinations where multiple tool calls share the same ID
    const captureToolResult = (msg: Message, index: number) => {
      if (msg.role === 'tool' && msg.tool_call_id) {
        if (!isMapCloned) {
          toolResultsMap = new Map(toolResultsMap);
          isMapCloned = true;
        }

        // Handle potential duplicate IDs by appending a sequence number if the key already exists
        let key = msg.tool_call_id;
        let seq = 1;
        while (toolResultsMap.has(key)) {
          key = `${msg.tool_call_id}_dup${seq}`;
          seq++;
        }

        toolResultsMap.set(key, msg);
        newLastToolResultIndex = Math.max(newLastToolResultIndex, index);
      }
    };

    // 5. Process new/changed messages
    // Start index is the end index of the last reused group (or 0)
    let i = reuseCount > 0 ? groupEndIndices[reuseCount - 1] : 0;

    while (i < messages.length) {
      const msg = messages[i];

      // Group consecutive failed tool results into a dedicated error group.
      // This must happen BEFORE the generic "skip standalone tool results" behavior.
      if (isToolErrorMessage(msg)) {
        const errorMessages: Message[] = [];
        let j = i;
        while (j < messages.length && isToolErrorMessage(messages[j])) {
          const toolMsg = messages[j];
          // Ensure tool error results are still captured in the toolResultsMap.
          captureToolResult(toolMsg, j);
          errorMessages.push(toolMsg);
          j++;
        }

        groupedMessages.push({
          type: 'tool_error_group',
          message: errorMessages[0],
          messages: errorMessages,
        });
        groupEndIndices.push(j);
        i = j;
        continue;
      }

      // Capture tool result in map (even if skipped later)
      if (msg.role === 'tool' && msg.tool_call_id) {
        const previous = i > 0 ? messages[i - 1] : undefined;
        const isImmediatelyAfterAssistantToolCall =
          previous?.role === 'assistant' &&
          Array.isArray(previous.tool_calls) &&
          previous.tool_calls.some((call) => call.id === msg.tool_call_id);

        if (!isImmediatelyAfterAssistantToolCall) {
          captureToolResult(msg, i);
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
            captureToolResult(messages[j], j);
            j++;
          }
        }

        // Group if there are any tool calls
        if (allToolCalls.length > 0) {
          // Pre-calculate results array to avoid O(K) mapping in render loop
          // Handle duplicate IDs by tracking usage count
          const idUsageCount = new Map<string, number>();

          const results = allToolCalls.map((call) => {
            const count = idUsageCount.get(call.id) || 0;
            idUsageCount.set(call.id, count + 1);

            const key = count === 0 ? call.id : `${call.id}_dup${count}`;
            return toolResultsMap.get(key);
          });

          groupedMessages.push({
            type: 'tool_group',
            message: msg,
            messages: groupMessages,
            toolGroup: { calls: allToolCalls, results },
          });
          groupEndIndices.push(j);
        } else {
          // Defensive fallback
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
    let finalToolResultsMap = toolResultsMap;

    // Only verify equality if we created a new map (isMapCloned)
    // If !isMapCloned, it IS the previous map.
    if (isMapCloned) {
      if (areMapsEqual(toolResultsMap, prevCache.toolResultsMap)) {
        finalToolResultsMap = prevCache.toolResultsMap;
      }
    }

    return {
      result: { groupedMessages, toolResultsMap: finalToolResultsMap },
      newCache: {
        messages,
        groupedMessages,
        groupEndIndices,
        toolResultsMap: finalToolResultsMap,
        lastToolResultIndex: newLastToolResultIndex,
      },
    };
  }, [messages]);

  // Update cache during render phase to ensure sync behavior.
  // The cache acts as an internal state tracking mechanism for memoization,
  // making it perfectly safe (and often preferred for caches) to update inline.
  if (cache.current !== calculation.newCache) {
    cache.current = calculation.newCache;
  }

  return calculation.result;
}
