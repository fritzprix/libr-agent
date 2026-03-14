import { extractMediaContent } from '@/lib/ai-service/utils';
import { getLogger } from '@/lib/logger';
import { llmConfigManager } from '@/lib/llm-config-manager';
import type { MCPContent, MCPTool } from '@/lib/mcp';
import { MessageNormalizer } from '@/lib/ai-service/message-normalizer';
import {
  type AIServiceConfig,
  type AIServiceProvider,
  AIServiceError,
  type IAIService,
  type ModelInfo,
  type SamplingOptions,
  type SamplingResponse,
} from './types';
import type { Message } from '@/models/chat';

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

  // Instance-level AbortController for stream cancellation
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

  /**
   * Sets the default configuration for the service.
   * @param config The new default configuration.
   */
  setDefaultConfig(config: AIServiceConfig): void {
    this.defaultConfig = { ...this.defaultConfig, ...config };
  }

  /**
   * Cancels any ongoing streaming request.
   */
  cancel(): void {
    if (!this.abortController.signal.aborted) {
      this.logger.info('Cancelling active stream');
      this.abortController.abort();
    }
    // Re-create the AbortController for the next request
    this.abortController = new AbortController();
  }

  /**
   * Returns the `AbortSignal` for the current request.
   * @returns The `AbortSignal`.
   * @protected
   */
  protected getAbortSignal(): AbortSignal {
    return this.abortController.signal;
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
   * Common validation logic for messages.
   * @param messages The messages to validate.
   * @throws `AIServiceError` if validation fails.
   * @protected
   */
  protected validateMessages(messages: Message[]): void {
    if (!messages || !Array.isArray(messages) || messages.length === 0) {
      throw new AIServiceError(
        'Messages array is empty or undefined',
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
   * Executes a function with retry logic.
   * @template T The type of the result of the operation.
   * @param fn The function to execute.
   * @returns A promise that resolves to the result of the function.
   * @protected
   */
  protected async withRetry<T>(fn: () => Promise<T>): Promise<T> {
    const maxRetries = this.defaultConfig.maxRetries ?? 3;
    let lastError: unknown;

    for (let i = 0; i <= maxRetries; i++) {
      try {
        return await fn();
      } catch (error: unknown) {
        lastError = error;

        // Don't retry on cancellation
        if (this.abortController.signal.aborted) {
          throw error;
        }

        // Only retry on certain errors (e.g., rate limits, network issues)
        if (this.shouldRetry(error)) {
          const delay =
            Math.pow(2, i) * (this.defaultConfig.retryDelay ?? 1000);
          this.logger.warn(
            `Retrying request (${i + 1}/${maxRetries}) after ${delay}ms...`,
          );
          await new Promise((resolve) => setTimeout(resolve, delay));
        } else {
          throw error;
        }
      }
    }

    if (lastError instanceof Error) {
      throw new AIServiceError(
        lastError.message,
        this.getProvider(),
        undefined,
        lastError,
      );
    }
    throw new AIServiceError(
      String(lastError),
      this.getProvider(),
      undefined,
      undefined,
    );
  }

  /**
   * Determines whether an error should trigger a retry.
   * @param error The error to check.
   * @returns `true` if the request should be retried, `false` otherwise.
   * @protected
   */
  protected shouldRetry(error: unknown): boolean {
    // Basic implementation: retry on 429 (Too Many Requests) and 5xx (Server Errors)
    const status = (error as { status?: number })?.status;
    if (status === 429 || (status >= 500 && status <= 599)) {
      return true;
    }
    return false;
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
  protected processMultiModalContent(content: MCPContent[]): Array<{
    type: string;
    text?: string;
    image?: string;
    audio?: string;
    mimeType?: string;
  }> {
    type MediaItem = {
      data?: string;
      mimeType?: string;
      source?: { data?: string; uri?: string; mimeType?: string };
    };
    return content.map((item) => {
      switch (item.type) {
        case 'text':
          return { type: 'text', text: (item as { text: string }).text };
        case 'image':
          return {
            type: 'image',
            image:
              (item as MediaItem).data ||
              (item as MediaItem).source?.data ||
              (item as MediaItem).source?.uri,
            mimeType:
              (item as MediaItem).mimeType ||
              (item as MediaItem).source?.mimeType,
          };
        case 'audio':
          return {
            type: 'audio',
            audio:
              (item as MediaItem).data ||
              (item as MediaItem).source?.data ||
              (item as MediaItem).source?.uri,
            mimeType:
              (item as MediaItem).mimeType ||
              (item as MediaItem).source?.mimeType,
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
   * @param context.messages The array of messages.
   * @param context.options The options for the request.
   * @param context.options.modelName The model name.
   * @param context.options.systemPrompt The system prompt.
   * @param context.options.availableTools Tools available.
   * @param context.options.config The service configuration.
   * @param context.config The current AI configuration.
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
        disableToolUse?: boolean;
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
      disableToolUse?: boolean;
    } = {},
  ): {
    config: AIServiceConfig;
    tools?: TProviderTool[];
    sanitizedMessages: Message[];
  } {
    this.validateMessages(messages);
    const config = this.mergeConfig(options);

    // Re-initialize AbortController for this call
    this.abortController = new AbortController();

    const tools = options.availableTools
      ? this.convertTools(options.availableTools)
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
   * Extracts image and audio items from a MCPContent array.
   * Used by provider conversion loops to identify media that needs special handling
   * since tool result messages can only carry text in the standard API format.
   * @param content The full content array from a tool result message.
   * @returns Only the image and audio MCPContent items.
   * @protected
   */
  protected extractMediaContent(content: MCPContent[]): MCPContent[] {
    return extractMediaContent(content);
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
   * @returns An async generator that yields chunks of the response as strings.
   */
  async *streamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
      disableToolUse?: boolean;
    } = {},
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
   * @returns An async generator that yields chunks of the response as strings.
   * @protected
   * @abstract
   */
  protected abstract doStreamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
      forceToolUse?: boolean;
      disableToolUse?: boolean;
    },
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
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
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
   * Compresses a slice of conversation messages into a single summary by
   * preserving the raw array and appending a strong summarisation instruction.
   * By preserving the raw `Message[]` structure along with the original tools
   * and system prompt, providers with automatic Prompt Caching (e.g. OpenAI,
   * Anthropic, Gemini) can achieve near 100% cache hit rates on the massive
   * preceding conversation history, reducing latency to seconds and massively
   * cutting costs.
   *
   * @param messages The messages to compress.
   * @param options Optional model name, config, system prompt, context, and tools.
   * @param options.modelName The name of the model.
   * @param options.config Optional configuration for the service.
   * @returns A promise that resolves to the summary text.
   */
  async compact(
    messages: Message[],
    options?: {
      modelName?: string;
      config?: AIServiceConfig;
      systemPrompt?: string;
      sessionContext?: string;
      availableTools?: MCPTool[];
    },
  ): Promise<string> {
    const compactMessages = [...messages];

    let instruction =
      'Summarise the previous conversation history concisely using structured Markdown.\n\n' +
      'Organize the summary into clear sections (e.g., "Key Decisions", "Context", "Tool Results", "Pending Actions") ' +
      'to preserve all information needed to continue the conversation effectively.\n\n' +
      'IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.';

    // Residual connection principle: when a prior compact-summary is present, the new
    // summary MUST be informationally equivalent to (old_summary ⊕ new_messages).
    // Every piece of information from the previous summary must carry forward unchanged —
    // treat it as an immutable residual, not a target for further compression.
    const firstMsg = compactMessages[0];
    if (firstMsg?.id.startsWith('compact-summary-')) {
      instruction =
        'The first message is a previously accumulated compact summary that represents ALL earlier conversation history.\n\n' +
        'CRITICAL RESIDUAL RULE: Every fact, decision, action, and context item recorded in that prior summary ' +
        'MUST be preserved verbatim or re-stated with equivalent fidelity in your new summary. ' +
        'Do NOT compress, omit, or paraphrase the prior summary — treat it as an immutable residual that carries forward in full. ' +
        'Your new summary = (prior summary, preserved completely) + (new messages, summarised).\n\n' +
        instruction;
    }

    const instructionMessage: Message = {
      id: `compaction_instruction_${Date.now()}`,
      sessionId: 'internal',
      threadId: 'internal',
      role: 'user', // "user" role is critical here so it follows valid standard tool sequences
      content: [{ type: 'text', text: instruction }],
      createdAt: new Date(),
    };
    compactMessages.push(instructionMessage);

    const { systemPrompt: effectiveSystemPrompt, messages: effectiveMessages } =
      this.prepareContextInjection(
        options?.systemPrompt,
        options?.sessionContext,
        compactMessages,
      );

    const streamGenerator = this.streamChat(effectiveMessages, {
      modelName: options?.modelName,
      systemPrompt: effectiveSystemPrompt,
      availableTools: options?.availableTools,
      config: options?.config,
      forceToolUse: false,
      disableToolUse: true, // Key command to underlying providers
    });

    let summaryText = '';
    for await (const chunk of streamGenerator) {
      if (this.getAbortSignal().aborted) {
        throw new Error('Compaction request aborted');
      }

      let parsedChunk: Record<string, unknown>;
      try {
        parsedChunk = JSON.parse(chunk);
      } catch {
        parsedChunk = { content: chunk };
      }

      if (parsedChunk.content && typeof parsedChunk.content === 'string') {
        summaryText += parsedChunk.content;
      }
    }

    if (!summaryText.trim()) {
      throw new AIServiceError(
        'compact() received an empty response from streamChat',
        this.getProvider(),
      );
    }

    return summaryText.trim();
  }

  /**
   * Merges the stable system prompt and volatile session context into the
   * provider's preferred injection channel.
   *
   * The default implementation concatenates both parts into the system prompt —
   * safe for all providers and preserves existing behaviour. Providers that
   * support automatic prefix caching (e.g. OpenAI) can override this to inject
   * `sessionContext` as an ephemeral tail message instead, keeping the system
   * prompt fully static and maximising cache hit rates.
   *
   * @param systemPrompt - Stable prompt (sections 1–3, cacheable).
   * @param sessionContext - Volatile context (sections 4–5, rebuilt per turn).
   * @param messages - Current conversation message stack.
   * @returns Effective `systemPrompt` and `messages` to pass to `streamChat`.
   */
  prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): { systemPrompt: string | undefined; messages: Message[] } {
    if (!sessionContext) {
      return { systemPrompt, messages };
    }
    const combined = [systemPrompt, sessionContext]
      .filter(Boolean)
      .join('\n\n');
    return { systemPrompt: combined, messages };
  }

  /**
   * Cleans up any resources used by the service instance.
   * @abstract
   */
  abstract dispose(): void;
}
