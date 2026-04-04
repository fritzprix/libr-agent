import { Ollama } from 'ollama/browser';
import type {
  ChatRequest,
  ListResponse,
  ModelResponse,
  Message as OllamaMessage,
  Tool,
} from 'ollama';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import { ModelInfo } from '../llm-config-manager';
import {
  AIServiceProvider,
  AIServiceConfig,
  ContextInjectionResult,
} from './types';
import { BaseAIService } from './base-service';
import { supportsThinking, getContextWindow } from './model-capabilities';
import {
  createEphemeralSessionContextInjection,
  formatSessionContextAsBackgroundReference,
} from './base-service-context';
import {
  convertMCPToolsToOllamaTools,
  convertToOllamaMessages,
  processChunk,
  getModelToolSupport,
  determineThinkParam,
  type Logger,
  type SimpleOllamaMessage,
} from './ollama-core';

const logger = getLogger('OllamaService');

// Adapter to convert Tauri logger to core Logger interface
const coreLogger: Logger = {
  debug: (message: string, ...args: unknown[]) =>
    logger.debug(message, ...args),
  info: (message: string, ...args: unknown[]) => logger.info(message, ...args),
  warn: (message: string, ...args: unknown[]) => logger.warn(message, ...args),
  error: (message: string, ...args: unknown[]) =>
    logger.error(message, ...args),
};

// Constants
const DEFAULT_MODEL = 'llama3.1';

// Internal Interfaces
/** @internal */
interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
}

/**
 * An AI service implementation for interacting with a local Ollama server.
 */
export class OllamaService extends BaseAIService<SimpleOllamaMessage, Tool> {
  private host: string;
  private ollamaClient: Ollama;

  /**
   * Initializes a new instance of the OllamaService.
   * @param apiKey The API key (not required for local Ollama, but kept for interface consistency).
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    // Local models can be slow, especially for initial loading or large context.
    // We increase the default timeout to 5 minutes (300000ms) if not explicitly provided.
    super(apiKey, {
      timeout: 300_000,
      ...config,
    });
    this.host = config?.baseUrl || 'http://127.0.0.1:11434';

    // Use native fetch for all modes - simpler and faster
    this.ollamaClient = new Ollama({ host: this.host });
    logger.info('Ollama service initialized', {
      host: this.host,
    });
  }

  static supportsToolsForModel(modelName: string): boolean {
    return getModelToolSupport(modelName);
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('llama-3.1-405b')) return 128000;
    if (lowerName.includes('llama-3.1')) return 128000;
    if (lowerName.includes('llama-3.2')) return 128000;
    if (lowerName.includes('mistral-nemo')) return 128000;
    if (lowerName.includes('command-r')) return 128000;
    return 32768;
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Ollama`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Ollama;
  }

  override prepareContextInjection(
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ): ContextInjectionResult {
    if (!sessionContext) {
      return { systemPrompt, sessionContext: undefined, messages };
    }

    logger.debug('Injecting Ollama session context as ephemeral tail message', {
      sessionContextLength: sessionContext.length,
    });

    return createEphemeralSessionContextInjection(
      systemPrompt,
      sessionContext,
      messages,
      {
        idPrefix: 'ollama-session-context',
        contentText: formatSessionContextAsBackgroundReference(sessionContext),
      },
    );
  }

  /**
   * Ollama runs locally and doesn't require API key validation.
   * Override base class validation to allow empty API keys.
   * @param apiKey Ignored for Ollama; present only for interface compatibility.
   * @protected
   */
  protected validateApiKey(apiKey: string): void {
    // Ollama doesn't require API key for local server
    // Allow empty or any string value
    void apiKey;
  }

  /**
   * @inheritdoc
   */
  convertTools(mcpTools: MCPTool[]): Tool[] {
    return convertMCPToolsToOllamaTools(mcpTools, coreLogger);
  }

  /**
   * Cancels any ongoing streams by calling the Ollama client's abort method.
   * This will abort all running requests on the client instance.
   */
  public cancel(): void {
    super.cancel(); // Call base implementation to set the abort signal
    this.logger.info('Calling Ollama client abort()');
    this.ollamaClient.abort();
  }

  /**
   * Fetches the list of available models directly from the Ollama server.
   * It uses the `ollama.list()` API to get the installed models.
   * @returns A promise that resolves to an array of `ModelInfo` objects.
   *          Returns an empty array if the server is unavailable.
   */
  async listModels(): Promise<ModelInfo[]> {
    try {
      logger.info('Fetching models from Ollama server...');

      const response: ListResponse = await this.withRetry(async () => {
        return await this.ollamaClient.list();
      });

      // Convert the ollama.list() response to our standard ModelInfo format
      // Use getContextWindow for dynamic context window detection
      const models: ModelInfo[] = await Promise.all(
        response.models.map(async (model: ModelResponse) => {
          const contextWindow = await getContextWindow(
            model.name,
            AIServiceProvider.Ollama,
            { apiBase: this.host },
          );

          return {
            id: model.name,
            name: model.name,
            contextWindow,
            supportReasoning: true,
            supportTools: this.getModelToolSupport(model.name),
            supportStreaming: true,
            cost: { input: 0, output: 0 },
            description: model.details?.family || model.name || 'Ollama model',
          };
        }),
      );

      logger.info(`Found ${models.length} models on Ollama server`);
      return models;
    } catch (error) {
      logger.error('Failed to fetch models from Ollama server:', error);

      // Return an empty array on error (e.g., server is off or connection fails)
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      logger.warn(
        `Ollama server not available (${errorMessage}), returning empty model list`,
      );
      return [];
    }
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Ollama doesn't support thinking fields in the same way Anthropic does
    if (message.thinking) {
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }

    return message;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return OllamaService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return OllamaService.estimateContextWindowForModel(modelName);
  }

  /**
   * Initiates a streaming chat session with the Ollama API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @yields A JSON string for each chunk of the response.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    const {
      config,
      tools: ollamaTools,
      sanitizedMessages,
    } = this.prepareStreamChat(messages, options);

    logger.info('🔵 Ollama doStreamChat called', {
      inputMessageCount: messages.length,
      hasSystemPrompt: !!options.systemPrompt,
      model: options.modelName || config.defaultModel || DEFAULT_MODEL,
      availableToolsCount: options.availableTools?.length ?? 0,
    });

    try {
      const ollamaMessages = this.convertMessages(
        sanitizedMessages,
        options.systemPrompt,
      );
      const model = options.modelName || config.defaultModel || DEFAULT_MODEL;

      logger.info('📨 Converted messages for Ollama', {
        originalCount: sanitizedMessages.length,
        convertedCount: ollamaMessages.length,
        model,
        toolCount: (ollamaTools ?? []).length,
        messageRoles: ollamaMessages.map((m) => m.role).join(','),
      });

      logger.debug('🔍 Ollama message details', {
        messages: ollamaMessages.map((m, idx) => ({
          index: idx,
          role: m.role,
          contentPreview:
            typeof m.content === 'string'
              ? m.content.substring(0, 50) + '...'
              : '[non-string content]',
          hasToolCalls: !!m.tool_calls,
        })),
      });

      // Prepare reasoning parameter based on config
      // Check if model actually supports thinking (optional, for better UX)
      const modelSupportsThinking = config.enableReasoning
        ? await supportsThinking(model, AIServiceProvider.Ollama, {
            apiBase: this.host,
          })
        : false;

      const thinkParam = determineThinkParam(
        config.enableReasoning ?? false,
        config.reasoningEffort,
        modelSupportsThinking,
        coreLogger,
      );

      const requestOptions: ChatRequest & { stream: true } = {
        model,
        messages: ollamaMessages as OllamaMessage[],
        stream: true,
        ...(thinkParam !== undefined && { think: thinkParam }), // Conditional inclusion
        tools: ollamaTools,
        keep_alive: '5m',
        options: {
          temperature: config.temperature || 0.7,
          num_predict: config.maxTokens || 4096,
        },
      };

      const stream = await this.withRetry(async () => {
        try {
          return await this.ollamaClient.chat(requestOptions);
        } catch (error: unknown) {
          // If model doesn't support granular think levels, retry with boolean true
          if (
            error instanceof Error &&
            error.message.includes('think value') &&
            error.message.includes('is not supported') &&
            requestOptions.think &&
            typeof requestOptions.think === 'string'
          ) {
            logger.warn(
              `Model ${model} doesn't support think level '${requestOptions.think}', falling back to boolean true`,
            );
            requestOptions.think = true;
            return await this.ollamaClient.chat(requestOptions);
          }
          throw error;
        }
      });

      if (this.getAbortSignal().aborted) {
        this.logger.debug('Stream aborted before iteration');
        return;
      }

      // Tool call accumulator for partial JSON handling
      const toolCallAccumulators = new Map<
        number,
        import('./ollama-core').OllamaToolCallAccumulator
      >();

      for await (const chunk of stream) {
        if (this.getAbortSignal().aborted) {
          this.logger.debug('Stream aborted during iteration');
          break;
        }

        // DIAGNOSTIC LOGGING: Log raw chunk from generator
        // This is high volume, so keep it at debug level
        if (typeof chunk === 'object') {
          // It's already an object from ollama library
          // logger.debug('Raw Ollama Chunk', { chunk });
        }

        const processedChunk = processChunk(
          chunk,
          coreLogger,
          toolCallAccumulators,
        );
        if (processedChunk) {
          if (processedChunk.content) {
            yield JSON.stringify({ content: processedChunk.content });
          }

          if (processedChunk.thinking) {
            yield JSON.stringify({ thinking: processedChunk.thinking });
          }

          if (processedChunk.tool_calls) {
            yield JSON.stringify({ tool_calls: processedChunk.tool_calls });
          }

          if (processedChunk.usage) {
            yield JSON.stringify({ usage: processedChunk.usage });
          }

          if (processedChunk.error) {
            logger.error('Error processing chunk', processedChunk.error);
          }
        }
      }

      // Cleanup: Check for incomplete tool calls
      if (toolCallAccumulators.size > 0) {
        for (const accumulator of toolCallAccumulators.values()) {
          if (!accumulator.yielded) {
            logger.warn('Incomplete tool call at stream end', {
              id: accumulator.id,
              name: accumulator.name,
              partialJson: accumulator.partialJson.substring(0, 200),
            });
          }
        }
        toolCallAccumulators.clear();
      }
    } catch (error: unknown) {
      // AbortError is expected on cancellation, handle it gracefully
      if (error instanceof Error && error.name === 'AbortError') {
        this.logger.debug('Ollama stream was aborted successfully.');
        return;
      }
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the Ollama API.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `SimpleOllamaMessage` objects.
   * @private
   */
  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): SimpleOllamaMessage[] {
    return convertToOllamaMessages(messages, systemPrompt, coreLogger);
  }

  /**
   * Checks if a given Ollama model likely supports tool use based on its name.
   * @param modelName The name of the model.
   * @returns True if the model is known to support tools, false otherwise.
   * @private
   */
  private getModelToolSupport(modelName: string): boolean {
    return getModelToolSupport(modelName);
  }

  /**
   * Performs a non-streaming text generation request using the Ollama API.
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
    const config = this.mergeConfig(options);
    const model = options?.modelName || config.defaultModel || '';
    const s = options?.samplingOptions;

    const response = await this.ollamaClient.chat({
      model,
      stream: false,
      messages: [{ role: 'user', content: prompt }],
      options: {
        num_predict: s?.maxTokens ?? config.maxTokens,
        temperature: s?.temperature ?? config.temperature,
        top_p: s?.topP,
        stop: s?.stopSequences,
      },
    });

    const text = response.message.content;

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: response.done ? 'stop' : 'length',
          usage:
            response.eval_count !== undefined &&
            response.prompt_eval_count !== undefined
              ? {
                  promptTokens: response.prompt_eval_count,
                  completionTokens: response.eval_count,
                  totalTokens: response.prompt_eval_count + response.eval_count,
                }
              : undefined,
          model: response.model,
        },
      },
    };
  }

  /**
   * @inheritdoc
   * @description The Ollama client does not require explicit resource cleanup.
   */
  dispose(): void {
    // Ollama client doesn't require explicit cleanup
    logger.info('Ollama service disposed');
  }
}
