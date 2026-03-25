import type { TextBlockParam } from '@anthropic-ai/sdk/resources/messages.mjs';
import type { TokenUsage } from '../types';
import type { AnthropicUsageWithCache } from './types';

export const VOLATILE_CONTEXT_MARKER = '# Current Context Information';

export function buildAnthropicSystemBlocks(
  systemPrompt: string | undefined,
): TextBlockParam[] | undefined {
  if (!systemPrompt) return undefined;

  const idx = systemPrompt.indexOf(VOLATILE_CONTEXT_MARKER);

  if (idx < 0) {
    return [
      {
        type: 'text',
        text: systemPrompt,
        cache_control: { type: 'ephemeral' },
      },
    ];
  }

  if (idx === 0) {
    return [{ type: 'text', text: systemPrompt }];
  }

  const stablePrefix = systemPrompt.slice(0, idx).trimEnd();
  const volatileSuffix = systemPrompt.slice(idx);

  return [
    {
      type: 'text',
      text: stablePrefix,
      cache_control: { type: 'ephemeral' },
    },
    {
      type: 'text',
      text: volatileSuffix,
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
