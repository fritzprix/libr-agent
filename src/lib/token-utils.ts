import { get_encoding } from '@dqbd/tiktoken';
import type { Message } from '@/models/chat';
import { llmConfigManager } from './llm-config-manager';
import { getLogger } from './logger';
import { AIServiceProvider } from './ai-service/types';

const logger = getLogger('token-utils');

/**
 * Estimates token count for a message using BPE (cl100k_base) encoding.
 */
export function estimateTokensBPE(message: Message): number {
  const text = `${message.role}: ${message.content ?? ''}`;
  const encoding = get_encoding('cl100k_base');
  const tokens = encoding.encode(text);
  encoding.free();
  return tokens.length;
}

/**
 * Returns a message array that fits within the model's context window with 10% margin.
 * For Anthropic providers, considers tool chain boundaries to include only complete chains.
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
 * Tool chain completeness check helper function
 * Checks if selected messages have tool_use without corresponding tool_result
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
 * Removes incomplete tool chains to keep only complete chains
 */
function removeIncompleteToolChains(messages: Message[]): Message[] {
  const toolUseIds = new Set<string>();
  const completedToolUseIds = new Set<string>();

  // Collect all tool_use IDs
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    }
  }

  // Collect completed tool_use IDs
  for (const msg of messages) {
    if (
      msg.role === 'tool' &&
      msg.tool_call_id &&
      toolUseIds.has(msg.tool_call_id)
    ) {
      completedToolUseIds.add(msg.tool_call_id);
    }
  }

  // 불완전한 tool 체인 제거
  const result: Message[] = [];
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      // 완료된 tool_calls만 유지
      const completedToolCalls = msg.tool_calls.filter((tc) =>
        completedToolUseIds.has(tc.id),
      );

      if (completedToolCalls.length > 0) {
        const processedMsg = { ...msg, tool_calls: completedToolCalls };
        result.push(processedMsg);
      } else {
        // tool_calls가 모두 불완전한 경우 메시지에서 tool_calls 제거
        const processedMsg = { ...msg };
        delete processedMsg.tool_calls;
        delete processedMsg.tool_use;
        result.push(processedMsg);
      }
    } else if (msg.role === 'tool' && msg.tool_call_id) {
      // 완료된 tool_use에 대응하는 tool_result만 포함
      if (completedToolUseIds.has(msg.tool_call_id)) {
        result.push(msg);
      }
    } else {
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
