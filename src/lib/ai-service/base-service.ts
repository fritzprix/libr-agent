import { Message } from '@/models/chat';
import {
  MCPTool,
  MCPContent,
  SamplingOptions,
  SamplingResponse,
} from '../mcp-types';
import {
  AIServiceConfig,
  AIServiceProvider,
  AIServiceError,
  IAIService,
} from './types';
import { ModelInfo, llmConfigManager } from '../llm-config-manager';
import { withRetry, withTimeout } from '../retry-utils';
import { convertMCPToolsToProviderTools } from './tool-converters';
import { MessageNormalizer } from './message-normalizer';
import { getLogger } from '../logger';

/**
 * An abstract base class that provides common functionality for all AI services.
 * It implements the `IAIService` interface and handles API key validation,
 * message validation, retry logic, and configuration merging.
 */
export abstract class BaseAIService implements IAIService {
  /**
   * The default configuration for the service.
   * @protected
   */
  protected defaultConfig: AIServiceConfig = {
    timeout: 30000,
    maxRetries: 3,
    retryDelay: 1000,
    maxTokens: 4096,
    temperature: 0.7,
  };

  /**
   * A logger instance for the base service.
   * @protected
   */
  protected logger = getLogger('BaseAIService');

  // NEW: Instance-level AbortController for stream cancellation
  protected abortController: AbortController = new AbortController();

  /**
   * Initializes a new instance of the `BaseAIService`.
   * @param apiKey The API key for the service.
   * @param config Optional configuration to override the defaults.
   */
  constructor(
    protected apiKey: string,
    protected config?: AIServiceConfig,
  ) {
    this.validateApiKey(apiKey);
    this.defaultConfig = { ...this.defaultConfig, ...config };
  }

  // NEW: Get current abort signal
  protected getAbortSignal(): AbortSignal {
    return this.abortController.signal;
  }

  // NEW: Cancel current stream
  public cancel(): void {
    if (!this.abortController.signal.aborted) {
      this.logger.info('Cancelling active stream');
      this.abortController.abort();
    } else {
      this.logger.debug('cancel() called but no active stream');
    }
  }

  /**
   * Validates the provided API key.
   * @param apiKey The API key to validate.
   * @throws `AIServiceError` if the API key is invalid.
   * @protected
   */
  protected validateApiKey(apiKey: string): void {
    if (!apiKey || typeof apiKey !== 'string' || apiKey.trim().length === 0) {
      throw new AIServiceError('Invalid API key provided', this.getProvider());
    }
  }

  /**
   * Validates an array of messages to ensure they conform to the required structure.
   * @param messages The array of messages to validate.
   * @throws `AIServiceError` or `Error` if the messages are invalid.
   * @protected
   */
  protected validateMessages(messages: Message[]): void {
    if (!Array.isArray(messages) || messages.length === 0) {
      throw new AIServiceError(
        'Messages array cannot be empty',
        this.getProvider(),
      );
    }
    messages.forEach((message) => {
      if (!message.id || typeof message.id !== 'string') {
        throw new Error('Message must have a valid id');
      }
      if (
        (!message.content &&
          (message.role === 'user' || message.role === 'system')) ||
        (typeof message.content !== 'string' && !Array.isArray(message.content))
      ) {
        throw new Error('Message must have valid content');
      }
      if (!['user', 'assistant', 'system', 'tool'].includes(message.role)) {
        throw new Error('Message must have a valid role');
      }
    });
  }

  /**
   * A wrapper around the `withRetry` utility that automatically uses the service's
   * default retry configuration and wraps errors in `AIServiceError`.
   * @template T The type of the result of the operation.
   * @param operation The asynchronous operation to execute.
   * @param maxRetries The maximum number of retries, overriding the default.
   * @returns A promise that resolves with the result of the successful operation.
   * @protected
   */
  protected async withRetry<T>(
    operation: () => Promise<T>,
    maxRetries: number = this.defaultConfig.maxRetries!,
  ): Promise<T> {
    try {
      return await withRetry(operation, {
        maxRetries,
        baseDelay: this.defaultConfig.retryDelay!,
        timeout: this.defaultConfig.timeout!,
        exponentialBackoff: true,
      });
    } catch (error) {
      throw new AIServiceError(
        (error as Error).message,
        this.getProvider(),
        undefined,
        error as Error,
      );
    }
  }

  /**
   * A simple wrapper around the `withTimeout` utility.
   * @template T The type of the result of the promise.
   * @param promise The promise to execute with a timeout.
   * @param timeoutMs The timeout in milliseconds.
   * @returns A promise that resolves with the result or rejects on timeout.
   * @protected
   */
  protected async withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
  ): Promise<T> {
    return withTimeout(promise, timeoutMs);
  }

  /**
   * Processes an array of `MCPContent` parts into a single string,
   * extracting only the text content.
   * @param content The array of `MCPContent` to process.
   * @returns A single string concatenating all text parts.
   * @protected
   */
  protected processMessageContent(content: MCPContent[]): string {
    // Extracts only the text from the MCPContent array
    return content
      .filter((item) => item.type === 'text')
      .map((item) => (item as { text: string }).text)
      .join('\n');
  }

  /**
   * Processes an array of `MCPContent` parts for a multimodal LLM,
   * handling both text and image content.
   * @param content The array of `MCPContent` to process.
   * @returns An array of objects suitable for a multimodal API,
   *          containing either text or image data.
   * @protected
   */
  protected processMultiModalContent(
    content: MCPContent[],
  ): Array<{ type: string; text?: string; image?: string }> {
    return content.map((item) => {
      switch (item.type) {
        case 'text':
          return { type: 'text', text: (item as { text: string }).text };
        case 'image':
          return {
            type: 'image',
            image:
              (
                item as {
                  data?: string;
                  source?: { data?: string; uri?: string };
                }
              ).data ||
              (item as { source?: { data?: string; uri?: string } }).source
                ?.data ||
              (item as { source?: { data?: string; uri?: string } }).source
                ?.uri,
          };
        default:
          return { type: 'text', text: `[${item.type}]` };
      }
    });
  }

  /**
   * A common error handling helper for streaming operations. It logs the error
   * and throws a standardized `AIServiceError`.
   * @param error The error that occurred.
   * @param context The context of the operation, including messages and options.
   * @throws `AIServiceError`
   * @protected
   */
  protected handleStreamingError(
    error: unknown,
    context: {
      messages: Message[];
      options: {
        modelName?: string;
        systemPrompt?: string;
        availableTools?: MCPTool[];
        config?: AIServiceConfig;
      };
      config: AIServiceConfig;
    },
  ): never {
    const serviceProvider = this.getProvider();
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    const errorStack = error instanceof Error ? error.stack : undefined;

    // NEW: Check if error is due to cancellation
    const isCancellation =
      this.abortController.signal.aborted ||
      (error instanceof Error &&
        (error.name === 'AbortError' ||
          error.message.includes('abort') ||
          error.message.includes('cancel')));

    if (isCancellation) {
      this.logger.info(`${serviceProvider} stream cancelled by user`);
      // We throw an error to stop the generator, but this is an expected "success" case
      // from the user's perspective. The calling code should catch this and handle it.
      throw new AIServiceError(
        `${serviceProvider} stream cancelled`,
        serviceProvider,
        undefined,
        error instanceof Error ? error : undefined,
      );
    }

    this.logger.error(`${serviceProvider} streaming failed`, {
      error: errorMessage,
      stack: errorStack,
      requestData: {
        model: context.options.modelName || context.config.defaultModel,
        messagesCount: context.messages.length,
        hasTools: !!context.options.availableTools?.length,
        systemPrompt: !!context.options.systemPrompt,
      },
    });

    throw new AIServiceError(
      `${serviceProvider} streaming failed: ${errorMessage}`,
      serviceProvider,
      undefined,
      error instanceof Error ? error : undefined,
    );
  }

  /**
   * Merges the provided options with the default service configuration.
   * @param options The options to merge.
   * @returns The merged `AIServiceConfig`.
   * @protected
   */
  protected mergeConfig(options?: {
    config?: AIServiceConfig;
  }): AIServiceConfig {
    return { ...this.defaultConfig, ...options?.config };
  }

  /**
   * A common preprocessing step for the `streamChat` method. It validates messages,
   * merges configuration, converts tools, and sanitizes messages.
   * @param messages The input messages.
   * @param options The options for the chat stream.
   * @returns An object containing the final configuration, converted tools, and sanitized messages.
   * @protected
   */
  protected prepareStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    } = {},
  ): {
    config: AIServiceConfig;
    tools?: unknown[];
    sanitizedMessages: Message[];
  } {
    this.validateMessages(messages);
    const config = this.mergeConfig(options);

    this.abortController = new AbortController();

    const tools = options.availableTools
      ? convertMCPToolsToProviderTools(
          options.availableTools,
          this.getProvider(),
        )
      : undefined;

    // Apply vendor-specific message sanitization
    const sanitizedMessages = this.sanitizeMessages(messages);

    return { config, tools, sanitizedMessages };
  }

  /**
   * Sanitizes messages for provider-specific compatibility.
   * The base implementation uses the `MessageNormalizer`, but services can override this
   * for custom sanitization logic.
   * @param messages The messages to sanitize.
   * @returns An array of sanitized messages.
   * @protected
   */
  protected sanitizeMessages(messages: Message[]): Message[] {
    return MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      this.getProvider(),
    );
  }

  /**
   * A template method for converting an array of `Message` objects into a format
   * suitable for a specific provider's API. It handles the system prompt and
   * iterates through messages, calling the abstract `convertSingleMessage` for each.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of provider-specific message objects.
   * @protected
   */
  protected convertMessagesTemplate(
    messages: Message[],
    systemPrompt?: string,
  ): unknown[] {
    const result: unknown[] = [];

    if (systemPrompt) {
      const systemMessage = this.createSystemMessage(systemPrompt);
      if (systemMessage) {
        result.push(systemMessage);
      }
    }

    for (const message of messages) {
      const converted = this.convertSingleMessage(message);
      if (converted) {
        result.push(converted);
      }
    }

    return result;
  }

  /**
   * Lists the models available for the service.
   * The default implementation returns models from the static `llmConfigManager`.
   * Services that support dynamic model discovery (e.g., Ollama) should override this method.
   * @returns A promise that resolves to an array of `ModelInfo` objects.
   */
  async listModels(): Promise<ModelInfo[]> {
    const provider = this.getProvider();
    const models = llmConfigManager.getModelsForProvider(provider);

    if (!models) {
      return [];
    }

    // Convert the record of models to an array.
    return Object.values(models);
  }

  // --- Abstract Methods for Subclasses ---

  /**
   * Creates a provider-specific system message object.
   * @param systemPrompt The text of the system prompt.
   * @returns A provider-specific representation of a system message.
   * @protected
   * @abstract
   */
  protected abstract createSystemMessage(systemPrompt: string): unknown;

  /**
   * Converts a single `Message` object into a provider-specific format.
   * @param message The message to convert.
   * @returns A provider-specific representation of the message.
   * @protected
   * @abstract
   */
  protected abstract convertSingleMessage(message: Message): unknown;

  /**
   * Initiates a streaming chat session with the AI service.
   * @param messages An array of messages representing the conversation history.
   * @param options Optional parameters for the chat session, including model name, tools, etc.
   * @returns An async generator that yields chunks of the response as strings.
   * @abstract
   */
  abstract streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  /**
   * Performs a non-streaming text generation (sampling) request.
   * The default implementation throws an error, as not all services may support this.
   * Subclasses should override this method if they support non-streaming sampling.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
   * @returns A promise that resolves to a `SamplingResponse`.
   */
  async sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
  ): Promise<SamplingResponse> {
    void prompt;
    void options;
    throw new AIServiceError(
      'sampleText not implemented for this service',
      this.getProvider(),
    );
  }

  /**
   * Gets the provider identifier for the service.
   * @returns The `AIServiceProvider` enum value for the current service.
   * @abstract
   */
  abstract getProvider(): AIServiceProvider;

  /**
   * Cleans up any resources used by the service instance.
   * @abstract
   */
  abstract dispose(): void;
}
