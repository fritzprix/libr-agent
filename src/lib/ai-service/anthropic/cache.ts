import type { TextBlockParam } from '@anthropic-ai/sdk/resources/messages.mjs';
import type { TokenUsage } from '../types';
import type { AnthropicUsageWithCache } from './types';

export function getAnthropicPromptTokens(
  usage: AnthropicUsageWithCache,
  previousDetails?: TokenUsage['details'],
): number {
  const inputTokens = usage.input_tokens ?? 0;
  const cacheCreationInputTokens =
    usage.cache_creation_input_tokens ??
    previousDetails?.cacheCreationInputTokens ??
    0;
  const cacheReadInputTokens =
    usage.cache_read_input_tokens ?? previousDetails?.cacheReadInputTokens ?? 0;

  return inputTokens + cacheCreationInputTokens + cacheReadInputTokens;
}

export function buildAnthropicSystemBlocks(
  systemPrompt: string | undefined,
): TextBlockParam[] | undefined {
  if (!systemPrompt) {
    return undefined;
  }

  return [
    {
      type: 'text',
      text: systemPrompt,
      cache_control: { type: 'ephemeral' },
    },
  ];
}

export function applyAnthropicMessageStartUsage(
  currentUsage: TokenUsage,
  usage: AnthropicUsageWithCache,
): TokenUsage {
  const promptTokens = getAnthropicPromptTokens(usage);
  const nextUsage: TokenUsage = {
    ...currentUsage,
    promptTokens,
    totalTokens: promptTokens + currentUsage.completionTokens,
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
      ? getAnthropicPromptTokens(usage, currentUsage.details)
      : currentUsage.promptTokens;
  const completionTokens = usage.output_tokens ?? 0;
  const nextUsage: TokenUsage = {
    ...currentUsage,
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
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

export function buildAnthropicPromptCacheMetadata(
  usage: AnthropicUsageWithCache,
  previousDetails?: TokenUsage['details'],
): {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  cachedPromptTokens?: number;
  cacheCreationInputTokens?: number;
  cacheReadInputTokens?: number;
} {
  const promptTokens = getAnthropicPromptTokens(usage, previousDetails);
  const completionTokens = usage.output_tokens ?? 0;
  const cacheCreationInputTokens =
    usage.cache_creation_input_tokens ??
    previousDetails?.cacheCreationInputTokens;
  const cacheReadInputTokens =
    usage.cache_read_input_tokens ?? previousDetails?.cacheReadInputTokens;

  return {
    promptTokens,
    completionTokens,
    totalTokens: promptTokens + completionTokens,
    cachedPromptTokens: cacheReadInputTokens ?? undefined,
    cacheCreationInputTokens: cacheCreationInputTokens ?? undefined,
    cacheReadInputTokens: cacheReadInputTokens ?? undefined,
  };
}
