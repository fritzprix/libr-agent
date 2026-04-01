import OpenAI from 'openai';

export interface OpenAIStreamUsage {
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
  prompt_tokens_details?: { cached_tokens?: number };
  prompt_cache_hit_tokens?: number;
  completion_tokens_details?: { reasoning_tokens?: number };
}

export function isOpenAIStreamUsage(
  value: unknown,
): value is OpenAIStreamUsage {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const obj = value as Record<string, unknown>;

  if (
    obj.prompt_tokens !== undefined &&
    typeof obj.prompt_tokens !== 'number'
  ) {
    return false;
  }
  if (
    obj.completion_tokens !== undefined &&
    typeof obj.completion_tokens !== 'number'
  ) {
    return false;
  }
  if (obj.total_tokens !== undefined && typeof obj.total_tokens !== 'number') {
    return false;
  }
  if (
    obj.prompt_cache_hit_tokens !== undefined &&
    typeof obj.prompt_cache_hit_tokens !== 'number'
  ) {
    return false;
  }

  if (obj.prompt_tokens_details !== undefined) {
    if (
      typeof obj.prompt_tokens_details !== 'object' ||
      obj.prompt_tokens_details === null
    ) {
      return false;
    }
    const details = obj.prompt_tokens_details as Record<string, unknown>;
    if (
      details.cached_tokens !== undefined &&
      typeof details.cached_tokens !== 'number'
    ) {
      return false;
    }
  }

  if (obj.completion_tokens_details !== undefined) {
    if (
      typeof obj.completion_tokens_details !== 'object' ||
      obj.completion_tokens_details === null
    ) {
      return false;
    }
    const details = obj.completion_tokens_details as Record<string, unknown>;
    if (
      details.reasoning_tokens !== undefined &&
      typeof details.reasoning_tokens !== 'number'
    ) {
      return false;
    }
  }

  return true;
}

export interface OpenAIResponseUsageDetails {
  prompt_tokens_details?: Record<string, unknown>;
  completion_tokens_details?: Record<string, unknown>;
  prompt_cache_hit_tokens?: number;
}

export interface OpenAIMessageFingerprint {
  role: string;
  contentLength: number;
  contentHash: string;
  contentTag?: 'regular' | 'session_context' | 'tool_result_media';
  toolCallCount: number;
  toolCallNames?: string[];
  toolCallHash?: string;
  toolCallIdHash?: string;
  toolCallId?: string;
}

export interface OpenAIPromptSnapshot {
  mode: 'stream' | 'non-stream';
  model: string;
  systemPromptLength: number;
  systemPromptHash: string;
  toolsHash: string;
  toolCount: number;
  messagesFingerprintHash: string;
  messageFingerprints: OpenAIMessageFingerprint[];
  promptCacheKey?: string;
  promptCacheRetention?: 'in_memory' | '24h';
  compatibleCachePrompt: boolean;
}

export type OpenAIStreamingRequest =
  OpenAI.Chat.Completions.ChatCompletionCreateParamsStreaming & {
    cache_prompt?: boolean;
    prompt_cache_key?: string;
    prompt_cache_retention?: 'in_memory' | '24h';
  };

export type OpenAINonStreamingRequest =
  OpenAI.Chat.Completions.ChatCompletionCreateParamsNonStreaming & {
    cache_prompt?: boolean;
    prompt_cache_key?: string;
    prompt_cache_retention?: 'in_memory' | '24h';
  };

export interface OpenAILoggerLike {
  debug(message: string, ...args: unknown[]): void;
  info(message: string, ...args: unknown[]): void;
  warn(message: string, ...args: unknown[]): void;
  error(message: string, ...args: unknown[]): void;
}
