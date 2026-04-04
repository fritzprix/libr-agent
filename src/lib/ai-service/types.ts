import type { ModelInfo } from '../llm-config-manager';
import type { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import type { Message } from '@/models/chat';

export type { ModelInfo, SamplingOptions, SamplingResponse };

export interface ContextInjectionResult {
  systemPrompt: string | undefined;
  sessionContext?: string;
  messages: Message[];
}

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
  /** The sampling temperature for the model. */
  temperature?: number;
  /** Safety settings for the model (e.g. Gemini). */
  safetySettings?: SafetySetting[];
  /** An array of tools available to the service. */
  tools?: MCPTool[];
  /** The base URL for the service API endpoint. */
  baseUrl?: string;
  /**
   * Explicitly enables the non-standard `cache_prompt` extension used by some
   * OpenAI-compatible backends such as llama.cpp.
   *
   * This does not control OpenAI's official prompt caching, which is automatic
   * on supported models and surfaces via usage.prompt_tokens_details.cached_tokens.
   * When undefined, compatible extensions may still auto-enable for clearly
   * non-OpenAI endpoints configured under the OpenAI provider.
   */
  enablePromptCache?: boolean;

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
   * Enable reasoning mode for supported models.
   * Per-conversation temporary setting (not global).
   *
   * Provider-specific parameters:
   * - Ollama: think: true | 'low' | 'medium' | 'high'
   * - OpenAI: reasoning_effort: 'low' | 'medium' | 'high' (o1/o3/o4 only)
   * - Anthropic: extended_thinking: true (Claude 3.5+)
   * - Gemini: thinkingConfig.thinkingBudget: number | -1 | 0
   *
   * @default false
   */
  enableReasoning?: boolean;

  /**
   * Reasoning depth level when reasoning is enabled.
   * - 'low': Fast, minimal reasoning (~1K tokens)
   * - 'medium': Balanced reasoning (~8K tokens, default)
   * - 'high': Deep reasoning (~24K tokens, higher cost)
   *
   * @default 'medium'
   */
  reasoningEffort?: 'low' | 'medium' | 'high';
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
  AIContextInjectionService,
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
