import type { ModelInfo } from '../llm-config-manager';
import type { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import type { ThinkingEffort } from './thinking-effort-mapping';

export type { ModelInfo, SamplingOptions, SamplingResponse };

export interface SafetySetting {
  category: string;
  threshold: string;
}

/**
 * Token usage metrics returned by LLM providers
 */
export interface TokenUsage {
  /** Input tokens (prompt) */
  promptTokens: number;
  /** Output tokens (completion) */
  completionTokens: number;
  /** Total tokens */
  totalTokens: number;
  /** Cached input tokens (prompt caching) */
  cachedPromptTokens?: number;
  /** Provider-specific timing details */
  details?: {
    /** Reasoning tokens (o1/o3 models) */
    reasoningTokens?: number;
    /** Prompt evaluation duration (ms) - Ollama */
    promptEvalDuration?: number;
    /** Evaluation duration (ms) - Ollama */
    evalDuration?: number;
    /** Total duration (ms) - Ollama */
    totalDuration?: number;
    /** Model load duration (ms) - Ollama */
    loadDuration?: number;
    /** Cache creation input tokens (Anthropic) */
    cacheCreationInputTokens?: number;
    /** Cache read input tokens (Anthropic) */
    cacheReadInputTokens?: number;
    /** Cached content token count (Gemini) */
    cachedContentTokenCount?: number;
    /** Thinking/reasoning token count (Gemini reasoning models) */
    thoughtsTokenCount?: number;
    /** Prompt cache hit tokens (Groq/DeepSeek) */
    prompt_cache_hit_tokens?: number;
    /** Time to first token (ms) - Client-side measurement for providers without native metrics */
    timeToFirstToken?: number;
  };
}

/**
 * Defines the configuration options for an AI service.
 */
export interface AIServiceConfig {
  /** The timeout for API requests in milliseconds. */
  timeout?: number;
  /** The maximum number of times to retry a failed request. */
  maxRetries?: number;
  /** The base delay in milliseconds between retries. */
  retryDelay?: number;
  /** The default model to use for the service if none is specified. */
  defaultModel?: string;
  /** The maximum number of tokens to generate in a response. */
  maxTokens?: number;
  /** Safety settings for the model (e.g. Gemini). */
  safetySettings?: SafetySetting[];
  /** An array of tools available to the service. */
  tools?: MCPTool[];
  /** The base URL for the service API endpoint. */
  baseUrl?: string;
  /** Whether the OpenAI provider should target a 3rd party compatible endpoint. */
  use3rdParty?: boolean;
  /** Optional custom model identifier for OpenAI-compatible 3rd party endpoints. */
  customModelId?: string;

  /**
   * Optional OpenAI prompt cache routing key for official OpenAI endpoints.
   *
   * When unset, providers may derive a stable key automatically for chat
   * conversations that benefit from prefix caching.
   */
  promptCacheKey?: string;

  /**
   * Optional retention policy for official OpenAI prompt caching.
   *
   * The public docs describe `in_memory` and `24h`. The JS SDK types may lag
   * behind the API, so this field is forwarded only for official OpenAI
   * endpoints.
   */
  promptCacheRetention?: 'in_memory' | '24h';

  /**
   * Optional number of leading chat messages to include in automatically
   * derived OpenAI prompt cache keys.
   *
   * This keeps the default cross-session prefix-sharing behavior unchanged when
   * unset, but allows stricter cache partitioning for compatible proxies that
   * expect stable leading message history in the cache key.
   */
  promptCachePrefixMessageCount?: number;

  /**
   * Optional sampling temperature override.
   * When unset, providers omit temperature so serving-engine defaults apply.
   */
  temperature?: number;

  /**
   * Thinking effort preset. Best-effort: mapped to provider-native params where
   * supported. Self-hosted OpenAI-compatible servers may ignore this or use
   * different fields per model; responses can still include thinking tokens.
   *
   * @default undefined (treated as `off`)
   */
  thinkingEffort?: ThinkingEffort;
}

/**
 * An enumeration of the supported AI service providers.
 */
export enum AIServiceProvider {
  Groq = 'groq',
  OpenAI = 'openai',
  Anthropic = 'anthropic',
  Gemini = 'gemini',
  Fireworks = 'fireworks',
  Cerebras = 'cerebras',
  Ollama = 'ollama',
  OpenRouter = 'openrouter',
  Empty = 'empty',
}

export type AIServiceErrorKind =
  | 'context_limit'
  | 'rate_limit'
  | 'authentication'
  | 'network'
  | 'invalid_request'
  | 'server'
  | 'unknown';

export interface AIServiceErrorMetadata {
  kind?: AIServiceErrorKind;
  retryable?: boolean;
  providerCode?: string | number;
  providerStatus?: string;
  rawPayload?: unknown;
}

/**
 * A custom error class for AI service-related errors.
 * It includes information about the provider and the original error.
 */
export class AIServiceError extends Error {
  /**
   * Initializes a new instance of the `AIServiceError`.
   * @param message The error message.
   * @param provider The AI service provider that threw the error.
   * @param statusCode The HTTP status code of the error response, if available.
   * @param originalError The original `Error` object, if available.
   */
  constructor(
    message: string,
    public provider: AIServiceProvider,
    public statusCode?: number,
    public originalError?: Error,
    public metadata: AIServiceErrorMetadata = {},
  ) {
    super(message);
    this.name = 'AIServiceError';
  }
}

export type {
  AICompletionExecutionService,
  AICompactOptions,
  AIContextCompactionService,
  AICompactionService,
  AIMessageSanitizationService,
  AIModelLookupService,
  AIModelDiscoveryService,
  AISampleTextOptions,
  AISamplingService,
  AIServiceLifecycle,
  AIStreamChatOptions,
  AIStreamingService,
  AIToolSupportService,
  IAIService,
} from './service-contracts';
