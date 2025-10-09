import type { ModelInfo } from '../llm-config-manager';
import type { MCPTool, SamplingOptions, SamplingResponse } from '../mcp-types';
import type { Message } from '@/models/chat';

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
  /** An array of tools available to the service. */
  tools?: MCPTool[];
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
    },
  ): AsyncGenerator<string, void, void>;

  /**
   * Performs a non-streaming text generation (sampling) request from a single prompt.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
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
   * Cancels any in-progress streaming requests initiated by `streamChat`.
   * Implementations should abort network requests and stop yielding further
   * values from `streamChat` as soon as possible.
   *
   * This method is idempotent - calling it multiple times or calling it
   * when no stream is active should be safe and have no effect.
   */
  cancel(): void;

  /**
   * Cleans up any resources used by the service instance.
   */
  dispose(): void;
}
