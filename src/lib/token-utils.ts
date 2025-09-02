import { get_encoding } from '@dqbd/tiktoken';
import type { Message } from '@/models/chat';
import { llmConfigManager } from './llm-config-manager';
import { getLogger } from './logger';

const logger = getLogger('token-utils');

/**
 * BPE(cl100k_base) 기준으로 메시지의 토큰 수를 추정합니다.
 */
export function estimateTokensBPE(message: Message): number {
  const text = `${message.role}: ${message.content ?? ''}`;
  const encoding = get_encoding('cl100k_base');
  const tokens = encoding.encode(text);
  encoding.free();
  return tokens.length;
}

/**
 * 모델의 contextWindow에서 10% 마진을 적용하여, 초과하지 않는 메시지 배열을 반환합니다.
 * Anthropic의 경우 tool 체인 경계를 고려하여 완전한 체인만 포함합니다.
 */
export function selectMessagesWithinContext(
  messages: Message[],
  providerId: string,
  modelId: string,
): Message[] {
  const modelInfo = llmConfigManager.getModel(providerId, modelId);
  if (!modelInfo) {
    logger.warn(
      `Could not find model info for provider: ${providerId}, model: ${modelId}. Returning all messages.`,
    );
    return messages;
  }

  const safeWindow = Math.floor(modelInfo.contextWindow * 0.9);
  let totalTokens = 0;
  const selected: Message[] = [];

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    const tokens = estimateTokensBPE(msg);

    if (totalTokens + tokens > safeWindow) {
      // Anthropic인 경우 tool 체인 경계 검사
      if (providerId === 'anthropic') {
        const hasIncompleteToolChain = checkIncompleteToolChain(selected, msg);
        if (hasIncompleteToolChain) {
          logger.info(
            'Adjusting context window to preserve tool chain integrity',
            {
              originalSelected: selected.length,
              contextWindow: safeWindow,
              totalTokens,
            },
          );
          // tool 체인을 완전하게 만들기 위해 일부 메시지 제거
          const adjustedSelected = removeIncompleteToolChains(selected);
          return adjustedSelected;
        }
      }

      logger.info(
        `Context window limit reached. Total tokens: ${totalTokens}, Safe window: ${safeWindow}`,
      );
      break;
    }

    selected.unshift(msg);
    totalTokens += tokens;
  }

  return selected;
}

/**
 * Tool 체인 완전성 검사 헬퍼 함수
 * 선택된 메시지에 tool_use가 있는데 대응하는 tool_result가 없는지 확인
 */
function checkIncompleteToolChain(
  selected: Message[],
  candidateMsg: Message,
): boolean {
  // 현재 선택된 메시지들에서 tool_use ID들 수집
  const toolUseIds = new Set<string>();
  for (const msg of selected) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    }
  }

  // candidate 메시지도 포함해서 검사
  if (candidateMsg.role === 'assistant' && candidateMsg.tool_calls) {
    candidateMsg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
  }

  // tool_result로 완료된 tool_use 확인
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

  // candidate 메시지도 포함해서 검사
  if (
    candidateMsg.role === 'tool' &&
    candidateMsg.tool_call_id &&
    toolUseIds.has(candidateMsg.tool_call_id)
  ) {
    completedToolUseIds.add(candidateMsg.tool_call_id);
  }

  // 불완전한 tool_use가 있는지 확인
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
 * 불완전한 tool 체인을 제거하여 완전한 체인만 남김
 */
function removeIncompleteToolChains(messages: Message[]): Message[] {
  const toolUseIds = new Set<string>();
  const completedToolUseIds = new Set<string>();

  // 모든 tool_use ID 수집
  for (const msg of messages) {
    if (msg.role === 'assistant' && msg.tool_calls) {
      msg.tool_calls.forEach((tc) => toolUseIds.add(tc.id));
    }
  }

  // 완료된 tool_use ID 수집
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
