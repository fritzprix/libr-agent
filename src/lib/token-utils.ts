import { get_encoding, type Tiktoken } from '@dqbd/tiktoken';
import type { Message } from '@/models/chat';
import { MCPTextContent } from '@/lib/mcp';
import { llmConfigManager } from './llm-config-manager';
import { getLogger } from './logger';
import { AIServiceProvider } from './ai-service/types';

const logger = getLogger('token-utils');

/**
 * Singleton tiktoken encoder for cl100k_base.
 * Creating/freeing a WASM instance on every call causes synchronous GC pressure
 * and main-thread jank. We keep one instance alive for the app lifetime.
 */
let _sharedEncoder: Tiktoken | null = null;

function getSharedEncoder(): Tiktoken | null {
  if (_sharedEncoder !== null) return _sharedEncoder;
  try {
    _sharedEncoder = get_encoding('cl100k_base');
    return _sharedEncoder;
  } catch {
    return null;
  }
}

/**
 * Estimates the token count for arbitrary text using the `cl100k_base`
 * Byte-Pair Encoding (BPE), which is a common encoding for many modern LLMs.
 * Falls back to character-based estimation if tiktoken fails (e.g., for non-OpenAI models).
 *
 * @param text The text to estimate the token count for.
 * @returns The estimated number of tokens.
 */
export function estimateTextTokens(text: string): number {
  try {
    const encoder = getSharedEncoder();
    if (encoder) {
      return encoder.encode(text).length;
    }
    // Fallback if WASM failed to initialize
    return Math.ceil(text.length / 4);
  } catch (error) {
    // Fallback: Use character-based estimation (~4 chars per token, conservative OpenAI estimate)
    // This handles cases where tiktoken WASM fails (e.g., Ollama models, WASM compatibility issues)
    logger.debug(
      'tiktoken encoding failed, using character-based fallback',
      error,
    );
    return Math.ceil(text.length / 4);
  }
}

/**
 * Estimates the token count for a given message using the `cl100k_base`
 * Byte-Pair Encoding (BPE), which is a common encoding for many modern LLMs.
 * Falls back to character-based estimation if tiktoken fails.
 *
 * @param message The message to estimate the token count for.
 * @returns The estimated number of tokens.
 */
export function estimateTokensBPE(message: Message): number {
  try {
    let contentText = '';
    if (Array.isArray(message.content)) {
      contentText = message.content
        .map((c) => {
          if (c.type === 'text') return c.text;
          // For resources, we might want to count the text content if available
          if (c.type === 'resource') return c.resource.text || '';
          return '';
        })
        .join('');
    }

    const text = `${message.role}: ${contentText}`;
    return estimateTextTokens(text);
  } catch (error) {
    // Fallback: Conservative estimate for message overhead + content length
    logger.debug(
      'Message token estimation failed, using character-based fallback',
      error,
    );
    let contentLength = 0;
    if (Array.isArray(message.content)) {
      contentLength = message.content.reduce((sum, c) => {
        if (c.type === 'text') return sum + c.text.length;
        if (c.type === 'resource') return sum + (c.resource.text?.length || 0);
        return sum;
      }, 0);
    }
    // Account for role prefix and formatting: ~10 chars + content
    return Math.ceil((10 + contentLength) / 4);
  }
}

/**
 * Splits assistant messages with excessive tool calls into multiple messages
 * to prevent context window issues and improve readability.
 *
 * When an assistant message contains more tool calls than the specified threshold,
 * this function creates multiple assistant messages with batched tool calls,
 * maintaining proper pairing with their corresponding tool response messages.
 *
 * @param messages The array of messages to process.
 * @param maxToolCallsPerMessage Maximum number of tool calls per assistant message (default: 4).
 * @returns A new array of messages with tool calls batched appropriately.
 *
 * @example
 * Input: [
 *   { role: 'assistant', tool_calls: [tc1, tc2, tc3, tc4, tc5, tc6] },
 *   { role: 'tool', tool_call_id: 'tc1', ... },
 *   { role: 'tool', tool_call_id: 'tc2', ... },
 *   ...
 * ]
 *
 * Output (with maxToolCallsPerMessage=4): [
 *   { role: 'assistant', tool_calls: [tc1, tc2, tc3, tc4], content: [...] },
 *   { role: 'tool', tool_call_id: 'tc1', ... },
 *   { role: 'tool', tool_call_id: 'tc2', ... },
 *   { role: 'tool', tool_call_id: 'tc3', ... },
 *   { role: 'tool', tool_call_id: 'tc4', ... },
 *   { role: 'assistant', tool_calls: [tc5, tc6], content: '[Continuing tool calls]' },
 *   { role: 'tool', tool_call_id: 'tc5', ... },
 *   { role: 'tool', tool_call_id: 'tc6', ... },
 * ]
 */
export function batchToolCallsInMessages(
  messages: Message[],
  maxToolCallsPerMessage: number = 4,
): Message[] {
  if (maxToolCallsPerMessage < 1) {
    logger.warn('Invalid maxToolCallsPerMessage, using default of 4', {
      provided: maxToolCallsPerMessage,
    });
    maxToolCallsPerMessage = 4;
  }

  const result: Message[] = [];
  const processedMessageIds = new Set<string>(); // Track which messages we've already added
  let batchCounter = 0;

  for (const msg of messages) {
    // Skip if already processed
    if (processedMessageIds.has(msg.id)) {
      continue;
    }

    // Only process assistant messages with tool calls exceeding threshold
    // CRITICAL: Do NOT batch if the message has a thinkingSignature.
    // A thinkingSignature implies an atomic reasoning turn where all tool calls
    // are authorized by that single signature. Splitting them would orphan
    // subsequent batches from the signature, causing API validation errors (e.g., Gemini).
    if (
      msg.role === 'assistant' &&
      msg.tool_calls &&
      msg.tool_calls.length > maxToolCallsPerMessage &&
      !msg.thinkingSignature
    ) {
      logger.info('Batching tool calls for assistant message', {
        messageId: msg.id,
        totalToolCalls: msg.tool_calls.length,
        maxPerMessage: maxToolCallsPerMessage,
        batchesNeeded: Math.ceil(
          msg.tool_calls.length / maxToolCallsPerMessage,
        ),
      });

      // Mark original message as processed
      processedMessageIds.add(msg.id);

      // Split tool calls into batches
      const batches: Message['tool_calls'][] = [];
      for (let i = 0; i < msg.tool_calls.length; i += maxToolCallsPerMessage) {
        batches.push(msg.tool_calls.slice(i, i + maxToolCallsPerMessage));
      }

      // Create separate assistant messages for each batch
      batches.forEach((batch, batchIndex) => {
        batchCounter++;
        const batchMsg: Message = {
          ...msg,
          id: `${msg.id}_batch_${batchIndex}`,
          tool_calls: batch,
          // Keep original content only for first batch, add continuation marker for others
          content:
            batchIndex === 0
              ? msg.content
              : [
                  {
                    type: 'text',
                    text: `[Continuing tool calls - Batch ${batchIndex + 1}/${batches.length}]`,
                  },
                ],
          // CRITICAL FIX: Only preserve thinkingSignature on the FIRST batch
          // Gemini's thought signature protocol requires signature only on the first function call
          // Duplicating it across batches causes "position 2" validation errors
          thinkingSignature:
            batchIndex === 0 ? msg.thinkingSignature : undefined,
        };
        result.push(batchMsg);

        // Find and add corresponding tool responses for this batch
        const toolCallIds = new Set(batch?.map((tc) => tc.id) ?? []);
        const batchResponses = messages.filter(
          (m) =>
            m.role === 'tool' &&
            m.tool_call_id &&
            toolCallIds.has(m.tool_call_id),
        );

        // Add tool responses immediately after this batch
        result.push(...batchResponses);

        // Mark these tool responses as processed
        batchResponses.forEach((r) => processedMessageIds.add(r.id));

        logger.debug('Created tool call batch', {
          batchId: batchMsg.id,
          batchIndex: batchIndex + 1,
          totalBatches: batches.length,
          toolCallsInBatch: batch?.length ?? 0,
          toolResponsesInBatch: batchResponses.length,
        });
      });
    } else {
      // Keep message as-is if it doesn't need batching
      result.push(msg);
      processedMessageIds.add(msg.id);
    }
  }

  if (batchCounter > 0) {
    logger.info('Tool call batching completed', {
      originalMessages: messages.length,
      batchedMessages: result.length,
      totalBatchesCreated: batchCounter,
      maxToolCallsPerMessage,
    });
  }

  return result;
}

/**
 * Selects a subset of messages from the end of an array that fits within a model's context window.
 * It calculates a token limit (either from `maxTokens` or 90% of the model's context window)
 * and includes messages from the most recent until the limit is reached.
 * For certain providers like Anthropic, it performs additional checks to ensure that
 * tool call chains are not broken.
 *
 * @param messages The array of messages to select from.
 * @param providerId The ID of the LLM provider.
 * @param modelId The ID of the model.
 * @param maxTokens An optional maximum number of tokens to include.
 * @param options.systemPrompt Optional system prompt to account for in token budget.
 * @param options.toolsJson Optional tools JSON string to account for in token budget.
 * @returns A new array of messages that fits within the context window.
 */
export function selectMessagesWithinContext(
  messages: Message[],
  providerId: string,
  modelId: string,
  maxTokens?: number,
  options?: {
    systemPrompt?: string;
    toolsJson?: string;
    maxMessages?: number;
    maxToolCallsPerMessage?: number; // NEW: Maximum tool calls per assistant message
  },
): Message[] {
  // STEP 1: Batch tool calls BEFORE any processing to prevent context window issues
  const batchedMessages = batchToolCallsInMessages(
    messages,
    options?.maxToolCallsPerMessage || 4,
  );

  // STEP 2: Calculate Token Budget & Pin First Message
  const modelInfo = llmConfigManager.getModel(providerId, modelId);
  const baseTokenLimit =
    maxTokens ?? Math.floor((modelInfo?.contextWindow ?? 128000) * 0.9);

  // Reserve tokens for system prompt and tools
  const systemPromptTokens = options?.systemPrompt
    ? estimateTextTokens(options.systemPrompt)
    : 0;
  const toolsTokens = options?.toolsJson
    ? estimateTextTokens(options.toolsJson)
    : 0;

  // Check if we should pin the first message (Crucial context)
  let pinnedMessage: Message | null = null;
  let pinnedMessageTokens = 0;

  if (batchedMessages.length > 0 && batchedMessages[0].role === 'user') {
    pinnedMessage = batchedMessages[0];
    pinnedMessageTokens = estimateTokensBPE(pinnedMessage);
    logger.info('📌 Pinning first message to context', {
      messageId: pinnedMessage.id,
      tokens: pinnedMessageTokens,
    });
  }

  const reservedTokens = systemPromptTokens + toolsTokens + pinnedMessageTokens;
  const tokenLimit = Math.max(1024, baseTokenLimit - reservedTokens);

  logger.info('📊 Starting message selection', {
    inputMessageCount: batchedMessages.length,
    maxMessages: options?.maxMessages || 'unlimited',
    tokenLimit,
    reservedTokens,
    pinnedMessageId: pinnedMessage?.id,
  });

  let totalTokens = 0;
  const selected: Message[] = [];

  // Iterate backwards from the most recent message
  for (let i = batchedMessages.length - 1; i >= 0; i--) {
    const msg = batchedMessages[i];

    // Skip the pinned message if we encounter it during backward iteration
    // (We will add it explicitly at the end)
    if (pinnedMessage && msg.id === pinnedMessage.id) {
      continue;
    }

    const tokens = estimateTokensBPE(msg);

    logger.info(
      `Processing message: ${msg.id}, tokens=${tokens}, accumulated=${totalTokens}, tokenLimit=${tokenLimit}, selectedCount=${selected.length}`,
    );

    // Check token limit
    if (totalTokens + tokens > tokenLimit) {
      // Providers that require strict tool chain boundary checking (no orphaned calls/results)
      if (
        providerId === AIServiceProvider.Anthropic ||
        providerId === AIServiceProvider.Gemini ||
        providerId === AIServiceProvider.OpenAI ||
        providerId === AIServiceProvider.Groq
      ) {
        // Check tool chain integrity before stopping
        const adjustedSelected = removeIncompleteToolChains(selected);

        // Add pinned message before returning
        if (pinnedMessage) {
          return prependPinnedMessage(pinnedMessage, adjustedSelected);
        }
        return adjustedSelected;
      }
      break;
    }

    // Check message count limit
    if (
      options?.maxMessages &&
      selected.length >= options.maxMessages - (pinnedMessage ? 1 : 0)
    ) {
      logger.info(
        `✂️ Message count limit reached - applying windowSize constraint`,
        {
          selectedCount: selected.length,
          windowSize: options.maxMessages,
          totalAvailableMessages: messages.length,
          trimmedMessageCount: messages.length - selected.length,
        },
      );

      if (
        providerId === AIServiceProvider.Anthropic ||
        providerId === AIServiceProvider.Gemini ||
        providerId === AIServiceProvider.OpenAI ||
        providerId === AIServiceProvider.Groq
      ) {
        const adjustedSelected = removeIncompleteToolChains(selected);
        if (pinnedMessage) {
          return prependPinnedMessage(pinnedMessage, adjustedSelected);
        }
        return adjustedSelected;
      }
      break;
    }

    selected.unshift(msg);
    totalTokens += tokens;
  }

  logger.info('✅ Message selection complete', {
    inputCount: batchedMessages.length,
    selectedCount: selected.length,
    trimmedCount: batchedMessages.length - selected.length,
    totalTokens,
    tokenLimit,
    maxMessages: options?.maxMessages || 'unlimited',
  });

  // Add the pinned message to the start
  if (pinnedMessage) {
    return prependPinnedMessage(pinnedMessage, selected);
  }

  return selected;
}

/**
 * Prepends a pinned message to the selected messages, handling adjacency.
 * If pinned message is User and next message is also User, they are merged.
 */
function prependPinnedMessage(
  pinnedMsg: Message,
  selectedMsgs: Message[],
): Message[] {
  if (selectedMsgs.length === 0) {
    return [pinnedMsg];
  }

  const firstSelected = selectedMsgs[0];

  // Check for User -> User adjacency
  if (pinnedMsg.role === 'user' && firstSelected.role === 'user') {
    // Merge them to avoid "User must be followed by Model" error
    // Explicitly cast to MCPTextContent to avoid type errors with Union types
    const separator: MCPTextContent = {
      type: 'text',
      text: '\n\n---\n\n(Merging context...)\n\n',
    };

    const content1 = Array.isArray(pinnedMsg.content)
      ? pinnedMsg.content
      : [{ type: 'text', text: String(pinnedMsg.content) } as MCPTextContent];

    const content2 = Array.isArray(firstSelected.content)
      ? firstSelected.content
      : [
          {
            type: 'text',
            text: String(firstSelected.content),
          } as MCPTextContent,
        ];

    const mergedContent = [...content1, separator, ...content2];

    const mergedMsg: Message = {
      ...pinnedMsg,
      id: `merged_${pinnedMsg.id}_${firstSelected.id}`,
      content: mergedContent,
    };

    logger.info('🔗 Merging pinned user message with adjacent user message', {
      pinnedId: pinnedMsg.id,
      adjacentId: firstSelected.id,
    });

    return [mergedMsg, ...selectedMsgs.slice(1)];
  }

  return [pinnedMsg, ...selectedMsgs];
}

/**
 * Filters an array of messages to remove any incomplete tool chains.
 * An incomplete chain is a `tool_calls` message without its corresponding `tool` result message.
 * This function ensures that only complete request/response pairs for tools are kept.
 *
 * @param messages The array of messages to process.
 * @returns A new array of messages with incomplete tool chains removed or cleaned.
 * @private
 */
function removeIncompleteToolChains(messages: Message[]): Message[] {
  const toolUseIds = new Set<string>();
  const completedToolUseIds = new Set<string>();

  // First pass: collect all tool call IDs
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    }
  }

  // Second pass: collect the IDs of tool calls that have a corresponding result
  for (const msg of messages) {
    if (
      msg.role === 'tool' &&
      msg.tool_call_id &&
      toolUseIds.has(msg.tool_call_id)
    ) {
      completedToolUseIds.add(msg.tool_call_id);
    }
  }

  // Third pass: build the result array, filtering out incomplete chains
  const result: Message[] = [];
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      // Keep only the tool calls that have been completed
      const completedToolCalls = msg.tool_calls.filter((tc) =>
        completedToolUseIds.has(tc.id),
      );

      if (completedToolCalls.length > 0) {
        // If there are any completed calls, include the message with only those calls
        const processedMsg = { ...msg, tool_calls: completedToolCalls };
        result.push(processedMsg);
      } else {
        // If all tool calls in this message are incomplete, remove the tool_calls property entirely
        const processedMsg = { ...msg };
        delete processedMsg.tool_calls;
        delete processedMsg.tool_use; // Also remove legacy tool_use if present
        result.push(processedMsg);
      }
    } else if (msg.role === 'tool' && msg.tool_call_id) {
      // Only include tool result messages that correspond to a completed tool call
      if (completedToolUseIds.has(msg.tool_call_id)) {
        result.push(msg);
      }
    } else {
      // Keep all other messages
      result.push(msg);
    }
  }

  logger.info('Removed incomplete tool chains from context window', {
    originalMessages: messages.length,
    processedMessages: result.length,
    totalToolUses: toolUseIds.size,
    completedToolUses: completedToolUseIds.size,
  });

  return result;
}
