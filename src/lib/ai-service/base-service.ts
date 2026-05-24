import { getLogger } from '@/lib/logger';
import { llmConfigManager } from '@/lib/llm-config-manager';
import type { MCPContent, MCPTool } from '@/lib/mcp';
import {
  filterSystemErrors,
  repairMalformedToolCalls,
  validateToolCallPairing,
} from '@/lib/ai-service/message-normalizer';
import {
  type CompactOptions,
  type PrepareStreamChatOptions,
  type PrepareStreamChatResult,
  type SampleTextOptions,
  type StreamChatOptions,
  type StreamingErrorContext,
} from './base-service-shared';
import {
  shouldRetryRequest,
  throwStreamingError,
  withRetryPolicy,
} from './base-service-strategies';
import { compactMessages } from './base-service-compaction';
import { prepareStreamChatRequest } from './base-service-stream-preparation';
import {
  extractMediaContent as extractMediaParts,
  processMessageContent as stringifyMessageContent,
  processMultiModalContent as buildMultiModalContent,
} from '@/lib/ai-service/utils';
import {
  validateApiKey as validateServiceApiKey,
  validateMessages as validateServiceMessages,
  validateToolDefinition,
} from './base-service-validation';
import {
  type AIServiceConfig,
  type AIServiceProvider,
  AIServiceError,
  type IAIService,
  type ModelInfo,
  type SamplingResponse,
} from './types';
import type { Message } from '@/models/chat';

export { stableHashKeyPart, stableStringify } from './base-service-utils';

/**
 * An abstract base class that provides common functionality for all AI services.
 * It implements the `IAIService` interface and handles API key validation,
 * message validation, retry logic, and configuration merging.
 * @template TProviderMessage The type of message objects used by the provider's API.
 * @template TProviderTool The type of tool objects used by the provider's API.
 */
export abstract class BaseAIService<TProviderMessage, TProviderTool>
  implements IAIService
{
  /**
   * The default configuration for the service.
   * @protected
   */
  protected defaultConfig: AIServiceConfig = {
    timeout: 30000,
    maxRetries: 3,
    retryDelay: 1000,
    maxTokens: 8192,
  };

  /**
   * A logger instance for the base service.
   * @protected
   */
  protected logger = getLogger(this.getProvider());

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

  /**
   * Cancels any ongoing streaming request.
   */
  cancel(): void {
    this.logger.debug(
      'Ignoring service-level cancel; request-scoped signal owns cancellation',
    );
  }

  /**
   * Validates the provided API key.
   * @param apiKey The API key to validate.
   * @throws `AIServiceError` if the API key is invalid.
   * @protected
   */
  protected validateApiKey(apiKey: string): void {
    validateServiceApiKey(apiKey, this.getProvider());
  }

  /**
   * Common validation logic for messages.
   * @param messages The messages to validate.
   * @throws `AIServiceError` if validation fails.
   * @protected
   */
  protected validateMessages(messages: Message[]): void {
    validateServiceMessages(messages, this.getProvider());
  }

  /**
   * Validates the basic structure of an `MCPTool`.
   * @param tool The tool to validate.
   * @throws An error if the tool is missing required fields.
   * @protected
   */
  protected validateTool(tool: MCPTool): void {
    validateToolDefinition(tool, this.getProvider());
  }

  /**
   * Executes a function with retry logic.
   * @template T The type of the result of the operation.
   * @param fn The function to execute.
   * @returns A promise that resolves to the result of the function.
   * @protected
   */
  protected async withRetry<T>(
    fn: () => Promise<T>,
    abortSignal?: AbortSignal,
  ): Promise<T> {
    return withRetryPolicy({
      fn,
      config: this.defaultConfig,
      abortSignal,
      logger: this.logger,
      provider: this.getProvider(),
      shouldRetry: (error) => this.shouldRetry(error),
    });
  }

  /**
   * Determines whether an error should trigger a retry.
   * @param error The error to check.
   * @returns `true` if the request should be retried, `false` otherwise.
   * @protected
   */
  protected shouldRetry(error: unknown): boolean {
    return shouldRetryRequest(error);
  }

  /**
   * Processes an array of `MCPContent` parts into a single string,
   * extracting only the text content.
   * @param content The array of `MCPContent` to process.
   * @returns A single string concatenating all text parts.
   * @protected
   */
  protected processMessageContent(content: MCPContent[]): string {
    return stringifyMessageContent(content);
  }

  /**
   * Processes an array of `MCPContent` parts for a multimodal LLM,
   * handling both text and image content.
   * @param content The array of `MCPContent` to process.
   * @returns An array of objects suitable for a multimodal API,
   *          containing either text or image data.
   * @protected
   */
  protected processMultiModalContent(content: MCPContent[]): Array<{
    type: string;
    text?: string;
    image?: string;
    audio?: string;
    mimeType?: string;
  }> {
    return buildMultiModalContent(content);
  }

  /**
   * A common error handling helper for streaming operations. It logs the error
   * and throws a standardized `AIServiceError`.
   * @param error The error that occurred.
   * @param context The context of the operation, including messages and options.
   * @param context.messages The array of messages.
   * @param context.options The options for the request.
   * @param context.options.modelName The model name.
   * @param context.options.systemPrompt The system prompt.
   * @param context.options.availableTools Tools available.
   * @param context.options.config The service configuration.
   * @param context.options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @param context.config The current AI configuration.
   * @throws `AIServiceError`
   * @protected
   */
  protected handleStreamingError(
    error: unknown,
    context: StreamingErrorContext,
  ): never {
    throwStreamingError({
      error,
      context,
      abortSignal: context.options.signal,
      logger: this.logger,
      provider: this.getProvider(),
    });
  }

  /**
   * Merges the provided options with the default service configuration.
   * @param options The options to merge.
   * @param options.config Optional configuration for the service.
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
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @returns An object containing the final configuration, converted tools, and sanitized messages.
   * @protected
   */
  protected prepareStreamChat(
    messages: Message[],
    options: PrepareStreamChatOptions = {},
  ): PrepareStreamChatResult<TProviderTool> {
    this.validateMessages(messages);

    return prepareStreamChatRequest({
      messages,
      options,
      mergeConfig: (streamOptions) => this.mergeConfig(streamOptions),
      convertTools: (tools) => this.convertTools(tools),
      sanitizeMessages: (inputMessages) => this.sanitizeMessages(inputMessages),
    });
  }

  /**
   * Sanitizes messages for provider-specific compatibility.
   * The base implementation calls sanitizeSingleMessage for each message.
   * @param messages The messages to sanitize.
   * @returns An array of sanitized messages.
   */
  sanitizeMessages(messages: Message[]): Message[] {
    const validMessages = filterSystemErrors(messages);
    const repairedMessages = repairMalformedToolCalls(validMessages);
    const processedMessages = validateToolCallPairing(repairedMessages);

    return processedMessages
      .map((msg) => this.sanitizeSingleMessage(msg))
      .filter((msg): msg is Message => msg !== null);
  }

  /**
   * Extracts image and audio items from a MCPContent array.
   * Used by provider conversion loops to identify media that needs special handling
   * since tool result messages can only carry text in the standard API format.
   * @param content The full content array from a tool result message.
   * @returns Only the image and audio MCPContent items.
   * @protected
   */
  protected extractMediaContent(content: MCPContent[]): MCPContent[] {
    return extractMediaParts(content);
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
   * Converts an array of `Message` objects into a format suitable for a specific provider's API.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend or include.
   * @returns An array of provider-specific message objects.
   * @protected
   * @abstract
   */
  protected abstract convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): TProviderMessage[];

  /**
   * Initiates a streaming chat session with the AI service.
   * This method wraps the `doStreamChat` method to provide common logging functionality.
   * @param messages An array of messages representing the conversation history.
   * @param options Optional parameters for the chat session, including model name, tools, etc.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @returns An async generator that yields chunks of the response as strings.
   */
  async *streamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    const provider = this.getProvider();
    const model =
      options.modelName || options.config?.defaultModel || 'unknown-model';

    this.logger.info(`[${provider}] streamChat CALL START`, {
      model,
      messagesCount: messages.length,
      toolsCount: options.availableTools?.length || 0,
      systemPromptLength: options.systemPrompt?.length,
      reasoningEnabled: options.config?.enableReasoning,
    });

    // Accumulate the full response for logging
    let accumulatedResponse = '';

    try {
      const start = Date.now();
      const generator = this.doStreamChat(messages, options);

      for await (const chunk of generator) {
        // Attempt to extract content for valid JSON chunks
        try {
          const parsed = JSON.parse(chunk);
          if (parsed.content) accumulatedResponse += parsed.content;
          // You might also want to track tool calls or thinking, but content is primary for "result"
        } catch {
          // If not JSON, just append raw (though it should be JSON)
          if (accumulatedResponse.length < 5000) {
            accumulatedResponse += chunk;
          }
        }
        yield chunk;
      }

      const duration = Date.now() - start;
      this.logger.info(`[${provider}] streamChat CALL END`, {
        model,
        durationMs: duration,
        responseLength: accumulatedResponse.length,
        responsePreview: accumulatedResponse.slice(0, 200),
      });
    } catch (error) {
      this.logger.error(`[${provider}] streamChat CALL ERROR`, error);
      throw error;
    }
  }

  /**
   * Wraps a streaming generator with Time-To-First-Token (TTFT) measurement.
   * Used by providers that don't provide native prefill timing metrics.
   * @param generator The underlying streaming generator to wrap
   * @returns A new generator that yields TTFT usage as the first chunk
   * @protected
   */
  protected async *streamChatWithTTFT(
    generator: AsyncGenerator<string, void, void>,
  ): AsyncGenerator<string, void, void> {
    const startTime = performance.now();
    let firstChunkReceived = false;

    for await (const chunk of generator) {
      if (!firstChunkReceived) {
        const ttft = performance.now() - startTime;
        firstChunkReceived = true;

        // Yield TTFT metric as the first chunk.
        // Only yield details — yielding zero token counts would briefly reset the
        // gauge to 0% before the real usage chunk arrives at the end of the stream.
        yield JSON.stringify({
          usage: { details: { timeToFirstToken: ttft } },
        });

        this.logger.debug('TTFT measured', {
          provider: this.getProvider(),
          ttftMs: ttft.toFixed(2),
        });
      }
      yield chunk;
    }
  }

  /**
   * Abstract method that performs the actual provider-specific streaming.
   * Must be implemented by subclasses.
   * @param messages An array of messages representing the conversation history.
   * @param options Optional parameters for the chat session.
   * @param options.modelName The name of the model.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @param options.config Optional configuration for the service.
   * @param options.forceToolUse Whether to force the model to use tools.
   * @param options.disableToolUse Whether to explicitly disable tool usage for this request.
   * @returns An async generator that yields chunks of the response as strings.
   * @protected
   * @abstract
   */
  protected abstract doStreamChat(
    messages: Message[],
    options?: StreamChatOptions,
  ): AsyncGenerator<string, void, void>;

  /**
   * Performs a non-streaming text generation (sampling) request.
   * The default implementation throws an error, as not all services may support this.
   * Subclasses should override this method if they support non-streaming sampling.
   * @param prompt The prompt to send to the model.
   * @param options Optional parameters for the sampling request.
   * @param options.modelName The name of the model.
   * @param options.samplingOptions The options used for text generation sampling.
   * @param options.config Optional configuration for the service.
   * @returns A promise that resolves to a `SamplingResponse`.
   */
  async sampleText(
    prompt: string,
    options?: SampleTextOptions,
  ): Promise<SamplingResponse> {
    // Deliberately unused — subclasses override this method.
    // The parameters exist only to satisfy the IAIService interface contract.
    void [prompt, options];
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
   * Converts an array of MCPTool objects to the provider-specific format.
   * Each service class must implement this to return the correct tool representation.
   * @param mcpTools The array of MCPTool objects to convert.
   * @returns An array of tools in the provider-specific format.
   * @abstract
   */
  abstract convertTools(mcpTools: MCPTool[]): TProviderTool[];

  /**
   * @inheritdoc
   */
  abstract sanitizeSingleMessage(message: Message): Message | null;

  /**
   * @inheritdoc
   */
  abstract supportsTools(modelName: string): boolean;

  /**
   * @inheritdoc
   */
  abstract estimateContextWindow(modelName: string): number;

  /**
   * Compresses a slice of conversation messages into a single summary string
   * by calling `sampleText()` internally. The default implementation in
   * `BaseAIService` builds a plain-text summarisation prompt; individual
   * providers may override for cost or caching optimisations.
   * @param messages The messages to compress.
   * @param options Optional model name, config, system prompt, and tools.
   * @param options.modelName The name of the model.
   * @param options.config Optional configuration for the service.
   * @param options.systemPrompt The system prompt.
   * @param options.availableTools Optional array of tools available to the model.
   * @returns A promise that resolves to the summary text.
   */
  async compact(
    messages: Message[],
    options?: CompactOptions,
  ): Promise<string> {
    return compactMessages(messages, {
      options,
      streamChat: (compactInput, compactOptions) =>
        this.streamChat(compactInput, compactOptions),
      isAborted: () => options?.signal?.aborted ?? false,
      getProvider: () => this.getProvider(),
    });
  }

  /**
   * Cleans up any resources used by the service instance.
   */
  abstract dispose(): void;
}
