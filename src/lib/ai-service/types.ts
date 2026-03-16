import type { ModelInfo } from '../llm-config-manager';
import type { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import type { Message } from '@/models/chat';

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
  /** The sampling temperature for the model. */
  temperature?: number;
  /** Safety settings for the model (e.g. Gemini). */
  safetySettings?: SafetySetting[];
  /** An array of tools available to the service. */
  tools?: MCPTool[];
  /** The base URL for the service API endpoint. */
  baseUrl?: string;

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

/**
 * Defines the common interface that all AI services must implement.
 */
export interface IAIService {
  /**
   * Initiates a streaming chat session with the AI service.
   * @param messages An array of messages representing the conversation history.
   * @param options Optional parameters for the chat session, including model name, tools, etc.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to disable tool use.
   * @returns An async generator that yields chunks of the response as strings.
   */
  streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
      /**
       * Disable tool use entirely for this stream, overriding `availableTools`.
       * Useful when preserving the prompt cache prefix but strictly preventing the
       * model from using tools (e.g. during compaction summarization).
       */
      disableToolUse?: boolean;
    },
  ): AsyncGenerator<string, void, void>;

  /**
   * Performs a non-streaming text generation (sampling) request from a single prompt.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
   * @param options.modelName The name of the model.
   * @param options.samplingOptions The options used for text generation sampling.
   * @param options.config Optional configuration for the service.
   * @returns A promise that resolves to a `SamplingResponse`.
   */
  sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
  ): Promise<SamplingResponse>;

  /**
   * Returns the list of supported models for this service.
   * For services like OpenAI/Anthropic, this returns static config data.
   * For services like Ollama, this may query the server dynamically.
   * @returns A promise that resolves to an array of `ModelInfo` objects.
   */
  listModels(): Promise<ModelInfo[]>;

  /**
   * Converts an array of MCPTool objects to the provider-specific format.
   * Each service class implements this to return the correct tool representation.
   */
  convertTools(mcpTools: MCPTool[]): unknown[];

  /**
   * Checks if a model supports tool use.
   * @param modelName The name of the model to check.
   */
  supportsTools(modelName: string): boolean;

  /**
   * Estimates the context window size for a model.
   * @param modelName The name of the model.
   */
  estimateContextWindow(modelName: string): number;

  /**
   * Cancels any in-progress streaming requests initiated by `streamChat`.
   * Implementations should abort network requests and stop yielding further
   * values from `streamChat` as soon as possible.
   *
   * This method is idempotent - calling it multiple times or calling it
   * when no stream is active should be safe and have no effect.
   */
  cancel(): void;

  /**
   * Compresses a slice of conversation messages into a single summary string
   * by calling `sampleText()` internally. The default implementation in
   * `BaseAIService` builds a plain-text summarisation prompt; individual
   * providers may override for cost or caching optimisations.
   * @param messages The messages to compress.
   * @param options Optional model name and service configuration overrides.
   * @param options.modelName The name of the model.
   * @param options.config Optional configuration for the service.
   * @param options.systemPrompt The system prompt.
   * @param options.sessionContext The session context.
   * @param options.availableTools Optional array of tools available to the model.
   * @returns A promise that resolves to the summary text.
   */
  compact(
    messages: Message[],
    options?: {
      modelName?: string;
      config?: AIServiceConfig;
      systemPrompt?: string;
      sessionContext?: string;
      availableTools?: MCPTool[];
    },
  ): Promise<string>;

  /**
   * Merges the stable system prompt and volatile session context into the
   * provider's preferred injection channel before each LLM request.
   *
   * The default implementation (in `BaseAIService`) concatenates both parts
   * into a single system prompt string — safe for all providers. Individual
   * providers may override to inject `sessionContext` as an ephemeral tail
   * message instead, which keeps the system prompt fully static and maximises
   * automatic prefix-cache hit rates.
   *
   * @param systemPrompt - Stable system prompt (sections 1–3). Cacheable.
   * @param sessionContext - Volatile context (sections 4–5). Rebuilt per turn.
   * @param messages - Current conversation message stack, after context trimming.
   * @returns The effective system prompt and (possibly augmented) message list to
   *          pass to `streamChat`.
   */
  prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): { systemPrompt: string | undefined; messages: Message[] };

  /**
   * Cleans up any resources used by the service instance.
   */
  dispose(): void;

  /**
   * Sanitizes messages for provider-specific compatibility.
   * @param messages The messages to sanitize.
   * @returns An array of sanitized messages.
   */
  sanitizeMessages(messages: Message[]): Message[];

  /**
   * Sanitizes a single message based on the provider's requirements.
   * @param message The message to sanitize.
   * @returns The sanitized message, or null if it should be filtered out.
   */
  sanitizeSingleMessage(message: Message): Message | null;
}
