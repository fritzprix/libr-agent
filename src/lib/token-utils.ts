import { get_encoding } from '@dqbd/tiktoken';
import type { Message } from '@/models/chat';
import { llmConfigManager } from './llm-config-manager';
import { getLogger } from './logger';
import { AIServiceProvider } from './ai-service/types';

const logger = getLogger('token-utils');

/**
 * Estimates the token count for a given message using the `cl100k_base`
 * Byte-Pair Encoding (BPE), which is a common encoding for many modern LLMs.
 *
 * @param message The message to estimate the token count for.
 * @returns The estimated number of tokens.
 */
export function estimateTokensBPE(message: Message): number {
  const text = `${message.role}: ${message.content ?? ''}`;
  const encoding = get_encoding('cl100k_base');
  const tokens = encoding.encode(text);
  encoding.free();
  return tokens.length;
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
 * @returns A new array of messages that fits within the context window.
 */
export function selectMessagesWithinContext(
  messages: Message[],
  providerId: string,
  modelId: string,
  maxTokens?: number,
): Message[] {
  const modelInfo = llmConfigManager.getModel(providerId, modelId);
  if (!modelInfo) {
    logger.warn(
      `Could not find model info for provider: ${providerId}, model: ${modelId}. Returning all messages.`,
    );
    return messages;
  }

  const tokenLimit = maxTokens ?? Math.floor(modelInfo.contextWindow * 0.9);
  let totalTokens = 0;
  const selected: Message[] = [];

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    const tokens = estimateTokensBPE(msg);

    if (totalTokens + tokens > tokenLimit) {
      // Anthropic providers require tool chain boundary checking
      if (providerId === AIServiceProvider.Anthropic) {
        const hasIncompleteToolChain = checkIncompleteToolChain(selected, msg);
        if (hasIncompleteToolChain) {
          logger.info(
            'Adjusting context window to preserve tool chain integrity',
            {
              originalSelected: selected.length,
              contextWindow: tokenLimit,
              totalTokens,
            },
          );
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

    selected.unshift(msg);
    totalTokens += tokens;
  }

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
