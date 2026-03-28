import type { TextBlockParam } from '@anthropic-ai/sdk/resources/messages.mjs';
import type { TokenUsage } from '../types';
import type { AnthropicUsageWithCache } from './types';

export function buildAnthropicSystemBlocks(
  systemPrompt: string | undefined,
  sessionContext?: string,
): TextBlockParam[] | undefined {
  if (!systemPrompt && !sessionContext) {
    return undefined;
  }

  if (systemPrompt && !sessionContext) {
    return [
      {
        type: 'text',
        text: systemPrompt,
        cache_control: { type: 'ephemeral' },
      },
    ];
  }

  if (!systemPrompt && sessionContext) {
    return [{ type: 'text', text: sessionContext }];
  }

  return [
    {
      type: 'text',
      text: systemPrompt!,
      cache_control: { type: 'ephemeral' },
    },
    {
      type: 'text',
      text: sessionContext!,
    },
  ];
}

export function applyAnthropicMessageStartUsage(
  currentUsage: TokenUsage,
  usage: AnthropicUsageWithCache,
): TokenUsage {
  const nextUsage: TokenUsage = {
    ...currentUsage,
    promptTokens: usage.input_tokens ?? 0,
    totalTokens: (usage.input_tokens ?? 0) + currentUsage.completionTokens,
  };

  if (
    usage.cache_creation_input_tokens !== undefined ||
    usage.cache_read_input_tokens !== undefined
  ) {
    nextUsage.cachedPromptTokens = usage.cache_read_input_tokens ?? undefined;
    nextUsage.details = {
      ...nextUsage.details,
      cacheCreationInputTokens: usage.cache_creation_input_tokens ?? undefined,
      cacheReadInputTokens: usage.cache_read_input_tokens ?? undefined,
    };
  }

  return nextUsage;
}

export function applyAnthropicMessageDeltaUsage(
  currentUsage: TokenUsage,
  usage: AnthropicUsageWithCache,
): TokenUsage {
  const promptTokens =
    usage.input_tokens !== undefined && usage.input_tokens !== null
      ? usage.input_tokens
      : currentUsage.promptTokens;
  const completionTokens = usage.output_tokens ?? 0;

  return {
    ...currentUsage,
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
  };
}
