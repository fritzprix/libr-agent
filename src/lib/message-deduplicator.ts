import { Message } from '@/models/chat';
import { getLogger } from './logger';

const logger = getLogger('MessageDeduplicator');

export interface DeduplicationOptions {
  preserveRecentN: number;
  minMessageCount: number;
}

const DEFAULT_OPTIONS: DeduplicationOptions = {
  preserveRecentN: 3,
  minMessageCount: 10,
};

interface ToolCallPair {
  assistantMessage: Message;
  toolMessage: Message;
  hash: string;
}

/**
 * Creates a hash key for a tool call/response pair
 * Format: toolName::arguments::responseContent
 */
function createPairHash(
  toolName: string,
  toolArguments: string,
  responseContent: string,
): string {
  return `${toolName}::${toolArguments}::${responseContent}`;
}

/**
 * Extracts tool call/response pairs from messages
 * Returns pairs with their hash keys for deduplication
 */
function extractToolCallPairs(messages: Message[]): ToolCallPair[] {
  const pairs: ToolCallPair[] = [];

  for (let i = 0; i < messages.length - 1; i++) {
    const current = messages[i];
    const next = messages[i + 1];

    // Check if this is a tool call/response pair
    if (
      current.role === 'assistant' &&
      current.tool_calls?.length === 1 &&
      next.role === 'tool' &&
      next.tool_call_id === current.tool_calls[0].id
    ) {
      const toolCall = current.tool_calls[0];
      const responseText =
        next.content[0]?.type === 'text' ? next.content[0].text : '';

      const hash = createPairHash(
        toolCall.function.name,
        toolCall.function.arguments,
        responseText,
      );

      pairs.push({
        assistantMessage: current,
        toolMessage: next,
        hash,
      });
    }
  }

  return pairs;
}

/**
 * Deduplicates tool call/response pairs in a message array
 * Removes entire pairs (assistant + tool messages) when identical
 * Only processes messages within the compressible range
 */
function deduplicatePairs(messages: Message[]): Message[] {
  const pairs = extractToolCallPairs(messages);
  
  if (pairs.length === 0) {
    return messages;
  }

  // Track which messages to remove (using Set for O(1) lookup)
  const messagesToRemove = new Set<string>();
  
  // Track first occurrence of each hash and count duplicates
  const seenHashes = new Map<string, { count: number; firstPair: ToolCallPair }>();

  for (const pair of pairs) {
    const existing = seenHashes.get(pair.hash);
    
    if (existing) {
      // This is a duplicate - mark for removal
      messagesToRemove.add(pair.assistantMessage.id);
      messagesToRemove.add(pair.toolMessage.id);
      existing.count++;
    } else {
      // First occurrence - keep it
      seenHashes.set(pair.hash, { count: 1, firstPair: pair });
    }
  }

  // Update metadata on first occurrences if there were duplicates
  const result: Message[] = [];
  
  for (const message of messages) {
    if (messagesToRemove.has(message.id)) {
      continue; // Skip duplicate messages
    }

    // Check if this is a tool message from a first occurrence with duplicates
    if (message.role === 'tool') {
      const matchingPair = Array.from(seenHashes.values()).find(
        ({ firstPair }) => firstPair.toolMessage.id === message.id,
      );

      if (matchingPair && matchingPair.count > 1) {
        // Add dedup metadata to the first occurrence
        const updatedMessage: Message = {
          ...message,
          metadata: {
            ...message.metadata,
            dedupCount: matchingPair.count,
          },
        };

        // Add dedup indicator to content text
        if (updatedMessage.content[0]?.type === 'text') {
          const originalText = updatedMessage.content[0].text;
          updatedMessage.content = [
            {
              type: 'text',
              text: `${originalText} (repeated ${matchingPair.count}x)`,
            },
          ];
        }

        result.push(updatedMessage);
        continue;
      }
    }

    result.push(message);
  }

  // Log deduplication stats
  const totalRemoved = messagesToRemove.size;
  const uniqueHashes = Array.from(seenHashes.values()).filter(
    (v) => v.count > 1,
  ).length;

  if (totalRemoved > 0) {
    logger.debug(
      `Deduplicated ${totalRemoved} messages from ${uniqueHashes} unique tool call patterns`,
    );
  }

  return result;
}

/**
 * Main deduplication function
 * Removes repeated identical tool call/response pairs to reduce token usage
 * 
 * Performance optimizations:
 * - Early exit for small message arrays (< minMessageCount)
 * - Preserves recent N messages untouched (active context)
 * - O(n) time complexity using hash-based comparison
 * 
 * Safety guarantees:
 * - Never orphans tool messages (removes pairs atomically)
 * - Preserves tool_call_id integrity
 * - Adds metadata to track deduplication count
 * 
 * @param messages - Original message array
 * @param options - Configuration options
 * @returns Deduplicated message array
 */
export function deduplicateToolCallPairs(
  messages: Message[],
  options?: Partial<DeduplicationOptions>,
): Message[] {
  const opts: DeduplicationOptions = {
    ...DEFAULT_OPTIONS,
    ...options,
  };

  // Early exit for small message arrays
  if (messages.length < opts.minMessageCount) {
    return messages;
  }

  // Split messages: compressible (old) vs preserve (recent)
  const preserveIndex = Math.max(0, messages.length - opts.preserveRecentN);
  const compressible = messages.slice(0, preserveIndex);
  const preserved = messages.slice(preserveIndex);

  // Deduplicate only compressible messages
  const deduplicated = deduplicatePairs(compressible);

  return [...deduplicated, ...preserved];
}
