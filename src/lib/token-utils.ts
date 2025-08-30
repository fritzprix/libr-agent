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

  // 10% 마진 적용
  const safeWindow = Math.floor(modelInfo.contextWindow * 0.9);

  let totalTokens = 0;
  const selected: Message[] = [];
  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];
    const tokens = estimateTokensBPE(msg);
    if (totalTokens + tokens > safeWindow) {
      logger.info(
        `Context window limit reached. Truncating messages. Total tokens: ${totalTokens}, Safe window: ${safeWindow}`,
      );
      break;
    }
    selected.unshift(msg);
    totalTokens += tokens;
  }
  return selected;
}
