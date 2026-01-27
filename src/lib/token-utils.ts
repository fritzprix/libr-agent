import { get_encoding } from '@dqbd/tiktoken';
import type { Message } from '@/models/chat';
import { llmConfigManager } from './llm-config-manager';
import { getLogger } from './logger';
import { AIServiceProvider } from './ai-service/types';

const logger = getLogger('token-utils');

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
    const encoding = get_encoding('cl100k_base');
    const tokens = encoding.encode(text);
    encoding.free();
    return tokens.length;
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
    if (
      msg.role === 'assistant' &&
      msg.tool_calls &&
      msg.tool_calls.length > maxToolCallsPerMessage
    ) {
      logger.info('Batching tool calls for assistant message', {
        messageId: msg.id,
        totalToolCalls: msg.tool_calls.length,
        maxPerMessage: maxToolCallsPerMessage,
        batchesNeeded: Math.ceil(msg.tool_calls.length / maxToolCallsPerMessage),
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

  const modelInfo = llmConfigManager.getModel(providerId, modelId);

  // If model info not found, apply message count limit only (no token-based truncation)
  if (!modelInfo) {
    logger.warn(
      `Could not find model info for provider: ${providerId}, model: ${modelId}. Applying message count limit only.`,
    );

    // If no maxMessages specified, return all messages
    if (!options?.maxMessages) {
      logger.info('No message count limit specified, returning all messages', {
        inputMessageCount: batchedMessages.length,
      });
      return batchedMessages;
    }

    // Apply simple message count-based truncation
    logger.info('📊 Starting message selection (count-based only)', {
      inputMessageCount: batchedMessages.length,
      maxMessages: options.maxMessages,
      provider: providerId,
      model: modelId,
    });

    const selected = batchedMessages.slice(-options.maxMessages);

    logger.info('✅ Message selection complete (count-based)', {
      inputCount: batchedMessages.length,
      selectedCount: selected.length,
      trimmedCount: batchedMessages.length - selected.length,
      maxMessages: options.maxMessages,
    });

    return selected;
  }

  const baseTokenLimit = maxTokens ?? Math.floor(modelInfo.contextWindow * 0.9);

  // Reserve tokens for system prompt and tools
  const systemPromptTokens = options?.systemPrompt
    ? estimateTextTokens(options.systemPrompt)
    : 0;
  const toolsTokens = options?.toolsJson
    ? estimateTextTokens(options.toolsJson)
    : 0;

  const reservedTokens = systemPromptTokens + toolsTokens;

  const tokenLimit = Math.max(1024, baseTokenLimit - reservedTokens); // Keep at least 1K for messages

  logger.info('📊 Starting message selection', {
    inputMessageCount: batchedMessages.length,
    maxMessages: options?.maxMessages || 'unlimited',
    provider: providerId,
    model: modelId,
  });

  logger.debug('Token budget allocation', {
    contextWindow: modelInfo.contextWindow,
    baseLimit: baseTokenLimit,
    systemPromptTokens,
    toolsTokens,
    reservedTokens,
    availableForMessages: tokenLimit,
  });
  let totalTokens = 0;
  const selected: Message[] = [];

  for (let i = batchedMessages.length - 1; i >= 0; i--) {
    const msg = batchedMessages[i];
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
        const hasIncompleteToolChain = checkIncompleteToolChain(selected, msg);
        if (hasIncompleteToolChain) {
          logger.info('Skipping message that would break tool chain boundary', {
            messageId: msg.id,
          });
          // Remove incomplete tool chains to maintain integrity
          const adjustedSelected = removeIncompleteToolChains(selected);
          return adjustedSelected;
        }
      }

      logger.info(
        `Context window limit reached. Total tokens: ${totalTokens}, Token limit: ${tokenLimit}`,
      );
      break;
    }

    // Check message count limit
    if (options?.maxMessages && selected.length >= options.maxMessages) {
      // Don't break tool chains even when hitting message count limit
      // If adding this message would complete a tool chain, allow it?
      // Or strictly cut off?
      // Strict cut off might break tool chains.
      // Let's apply the same checkIncompleteToolChain logic if we are about to stop.
      // But we iterate backwards. We are adding messages.
      // If we stop here, 'selected' contains the most recent N messages.
      // We are *omitting* 'msg' and everything before it.
      // If 'msg' is a Tool Call and the *next* message in 'selected' (which was 'i+1') is a Tool Result, we have an issue?
      // No, if 'msg' is Tool Call, and we omit it, then the Tool Result in 'selected' becomes orphaned.
      // So if we break due to Max Messages, we must perform the same tool chain integrity check/cleanup on 'selected'.

      if (
        providerId === AIServiceProvider.Anthropic ||
        providerId === AIServiceProvider.Gemini ||
        providerId === AIServiceProvider.OpenAI ||
        providerId === AIServiceProvider.Groq
      ) {
        // We are about to exclude 'msg'.
        // Check if the current 'selected' needs cleanup.
        // Actually, we should check if omitting 'msg' leaves 'selected' in a bad state?
        // No, 'selected' is accumulated. The danger is that 'selected[0]' (the oldest included message) is a Tool Result,
        // and its corresponding Tool Call is 'msg' (which we are excluding).
        // In that case, 'selected[0]' is an orphaned Tool Result.
        // So we should run removeIncompleteToolChains on 'selected'.
      }

      logger.info(
        `✂️ Message count limit reached - applying windowSize constraint`,
        {
          selectedCount: selected.length,
          windowSize: options.maxMessages,
          totalAvailableMessages: messages.length,
          trimmedMessageCount: messages.length - selected.length,
        },
      );

      // Verify integrity of selected messages before returning
      if (
        providerId === AIServiceProvider.Anthropic ||
        providerId === AIServiceProvider.Gemini ||
        providerId === AIServiceProvider.OpenAI ||
        providerId === AIServiceProvider.Groq
      ) {
        // Check if the *first* message in selected (oldest) is a broken tool chain/result
        // Easier to just run the cleanup
        const adjustedSelected = removeIncompleteToolChains(selected);
        // If cleanup reduced count, maybe we could have added more?
        // But simplicity first.
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

  return selected;
}

/**
 * Checks if a set of selected messages, plus a candidate message, would result
 * in an incomplete tool chain (i.e., a `tool_calls` message without a corresponding
 * `tool` result message).
 *
 * @param selected The array of messages already selected for the context.
 * @param candidateMsg The next message being considered for inclusion.
 * @returns True if an incomplete tool chain is detected, false otherwise.
 * @private
 */
function checkIncompleteToolChain(
  selected: Message[],
  candidateMsg: Message,
): boolean {
  // Collect tool_use IDs from currently selected messages
  const toolUseIds = new Set<string>();
  for (const msg of selected) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    }
  }

  // Also include candidate message in the check
  if (candidateMsg.role === 'assistant' && candidateMsg.tool_calls) {
    candidateMsg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
  }

  // Identify completed tool_use with tool_result
  const completedToolUseIds = new Set<string>();
  for (const msg of selected) {
    if (
      msg.role === 'tool' &&
      msg.tool_call_id &&
      toolUseIds.has(msg.tool_call_id)
    ) {
      completedToolUseIds.add(msg.tool_call_id);
    }
  }

  // Also include candidate message in the check
  if (
    candidateMsg.role === 'tool' &&
    candidateMsg.tool_call_id &&
    toolUseIds.has(candidateMsg.tool_call_id)
  ) {
    completedToolUseIds.add(candidateMsg.tool_call_id);
  }

  // Check for incomplete tool_use
  const incompleteToolUses = Array.from(toolUseIds).filter(
    (id) => !completedToolUseIds.has(id),
  );

  if (incompleteToolUses.length > 0) {
    logger.debug('Incomplete tool chain detected', {
      totalToolUses: toolUseIds.size,
      completedToolUses: completedToolUseIds.size,
      incompleteToolUses: incompleteToolUses.length,
    });
    return true;
  }

  return false;
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
