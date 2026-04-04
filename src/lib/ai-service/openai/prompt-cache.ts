import type { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';

import type { Message } from '@/models/chat';

import { stableHashKeyPart, stableStringify } from '../base-service';
import { AIServiceProvider, type AIServiceConfig } from '../types';
import type {
  OpenAINonStreamingRequest,
  OpenAIStreamingRequest,
} from './types';

function buildStableMessagePrefixPayload(
  messages: Message[],
  prefixMessageCount: number,
): string | undefined {
  if (prefixMessageCount <= 0) {
    return undefined;
  }

  const stablePrefixMessages = messages.slice(0, prefixMessageCount);
  if (stablePrefixMessages.length === 0) {
    return undefined;
  }

  return stableStringify(
    stablePrefixMessages.map((message) => ({
      role: message.role,
      content: message.content,
      tool_calls: message.tool_calls,
      tool_call_id: message.tool_call_id,
      tool_use: message.tool_use,
      source: message.source,
      thinking: message.thinking,
      thinkingSignature: message.thinkingSignature,
      metadata: message.metadata,
    })),
  );
}

export function isOfficialOpenAIEndpoint(
  provider: AIServiceProvider,
  config: AIServiceConfig,
): boolean {
  if (provider !== AIServiceProvider.OpenAI) {
    return false;
  }

  const baseUrl = config.baseUrl?.trim();
  if (!baseUrl) {
    return true;
  }

  try {
    const { hostname } = new URL(baseUrl);
    return hostname === 'api.openai.com';
  } catch {
    return false;
  }
}

export function shouldEnableCompatiblePromptCacheExtension(
  provider: AIServiceProvider,
  config: AIServiceConfig,
): boolean {
  if (config.enablePromptCache !== undefined) {
    return (
      config.enablePromptCache && !isOfficialOpenAIEndpoint(provider, config)
    );
  }

  if (provider !== AIServiceProvider.OpenAI) {
    return false;
  }

  const baseUrl = config.baseUrl?.trim();
  if (!baseUrl) {
    return false;
  }

  try {
    const { hostname } = new URL(baseUrl);
    return hostname !== 'api.openai.com';
  } catch {
    return false;
  }
}

export function withCompatiblePromptCache<T extends { cache_prompt?: boolean }>(
  request: T,
  provider: AIServiceProvider,
  config: AIServiceConfig,
): T {
  if (!shouldEnableCompatiblePromptCacheExtension(provider, config)) {
    return request;
  }

  return {
    ...request,
    cache_prompt: true,
  };
}

export function buildAutomaticPromptCacheKey(args: {
  model: string;
  systemPrompt?: string;
  messages?: Message[];
  tools?: OpenAIChatCompletionTool[];
  config?: AIServiceConfig;
}): string | undefined {
  if (!args.systemPrompt && !(args.tools && args.tools.length > 0)) {
    return undefined;
  }

  const toolsPayload = stableStringify(args.tools ?? []);
  const stableMessagePrefixPayload = buildStableMessagePrefixPayload(
    args.messages ?? [],
    Math.max(0, args.config?.promptCachePrefixMessageCount ?? 0),
  );

  return [
    'chat',
    args.model,
    stableHashKeyPart(args.systemPrompt ?? ''),
    stableHashKeyPart(toolsPayload),
    ...(stableMessagePrefixPayload
      ? [stableHashKeyPart(stableMessagePrefixPayload)]
      : []),
  ].join(':');
}

export function withOfficialPromptCaching<
  T extends {
    prompt_cache_key?: string;
    prompt_cache_retention?: 'in_memory' | '24h';
  },
>(
  request: T,
  provider: AIServiceProvider,
  config: AIServiceConfig,
  automaticPromptCacheKey?: string,
): T {
  if (!isOfficialOpenAIEndpoint(provider, config)) {
    return request;
  }

  const promptCacheKey = config.promptCacheKey ?? automaticPromptCacheKey;
  const promptCacheRetention = config.promptCacheRetention;

  if (!promptCacheKey && !promptCacheRetention) {
    return request;
  }

  return {
    ...request,
    ...(promptCacheKey ? { prompt_cache_key: promptCacheKey } : {}),
    ...(promptCacheRetention
      ? { prompt_cache_retention: promptCacheRetention }
      : {}),
  };
}

export function withPromptCaching<
  T extends OpenAIStreamingRequest | OpenAINonStreamingRequest,
>(
  request: T,
  provider: AIServiceProvider,
  config: AIServiceConfig,
  automaticPromptCacheKey?: string,
): T {
  const officialCachingRequest = withOfficialPromptCaching(
    request,
    provider,
    config,
    automaticPromptCacheKey,
  );
  return withCompatiblePromptCache(officialCachingRequest, provider, config);
}
