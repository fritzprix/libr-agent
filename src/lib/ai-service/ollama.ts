import { Ollama } from 'ollama/browser';
import type {
  ChatRequest,
  ListResponse,
  ModelResponse,
  Message as OllamaMessage,
} from 'ollama';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { ModelInfo } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { supportsThinking, getContextWindow } from './model-capabilities';
import {
  convertMCPToolsToOllamaTools,
  convertToOllamaMessages,
  convertMessage,
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
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
}

/**
 * An AI service implementation for interacting with a local Ollama server.
 */
export class OllamaService extends BaseAIService {
  private host: string;
  private ollamaClient: Ollama;

  /**
   * Initializes a new instance of the OllamaService.
   * @param apiKey The API key (not required for local Ollama, but kept for interface consistency).
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig & { host?: string }) {
    super(apiKey, config);
    this.host = config?.host || 'http://127.0.0.1:11434';
    this.ollamaClient = new Ollama({ host: this.host });
    logger.info('Ollama service initialized', {
      host: this.host,
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Ollama`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Ollama;
  }

  /**
   * Ollama runs locally and doesn't require API key validation.
   * Override base class validation to allow empty API keys.
   * @protected
   */
  protected validateApiKey(apiKey: string): void {
    // Ollama doesn't require API key for local server
    // Allow empty or any string value
    void apiKey;
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
   * Initiates a streaming chat session with the Ollama API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat.
   * @yields A JSON string for each chunk of the response.
   */
  async *streamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    const { config } = this.prepareStreamChat(messages, options);

    try {
      const ollamaMessages = this.convertToOllamaMessages(
        messages,
        options.systemPrompt,
      );
      const model = options.modelName || config.defaultModel || DEFAULT_MODEL;
      const ollamaTools = convertMCPToolsToOllamaTools(
        options.availableTools,
        coreLogger,
      );

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

      logger.info('Ollama API call:', {
        model,
        messagesCount: ollamaMessages.length,
        ollamaMessages,
        host: this.host,
        toolsCount: ollamaTools.length,
        reasoningEnabled: config.enableReasoning,
        reasoningEffort: config.reasoningEffort,
        thinkParam,
      });

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

      logger.info('Ollama request options:', { requestOptions });

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
        this.logger.info('Stream aborted before iteration');
        return;
      }

      for await (const chunk of stream) {
        if (this.getAbortSignal().aborted) {
          this.logger.info('Stream aborted during iteration');
          break;
        }

        logger.debug('Received chunk from Ollama', { chunk });

        const processedChunk = this.processChunk(chunk);
        if (processedChunk) {
          logger.debug('Processed chunk successfully', { processedChunk });
          yield processedChunk;
        } else {
          logger.debug('processChunk returned null');
        }
      }
    } catch (error: unknown) {
      // AbortError is expected on cancellation, handle it gracefully
      if (error instanceof Error && error.name === 'AbortError') {
        this.logger.info('Ollama stream was aborted successfully.');
        return;
      }
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * Processes a single chunk from the Ollama streaming response.
   * @param chunk The raw chunk from the stream.
   * @returns A JSON string representing the processed chunk, or null if empty.
   * @private
   */
  private processChunk(chunk: unknown): string | null {
    return processChunk(chunk, coreLogger);
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the Ollama API.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `SimpleOllamaMessage` objects.
   * @private
   */
  private convertToOllamaMessages(
    messages: Message[],
    systemPrompt?: string,
  ): SimpleOllamaMessage[] {
    return convertToOllamaMessages(messages, systemPrompt, coreLogger);
  }

  /**
   * Converts a single `Message` object to the corresponding `SimpleOllamaMessage` format.
   * @param message The message to convert.
   * @returns A `SimpleOllamaMessage` object, or null if the message is invalid.
   * @private
   */
  private convertMessage(message: Message): SimpleOllamaMessage | null {
    return convertMessage(message, coreLogger);
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
   * @inheritdoc
   * @description Creates an Ollama-compatible system message object.
   * @protected
   */
  protected createSystemMessage(systemPrompt: string): unknown {
    return {
      role: 'system',
      content: systemPrompt.trim(),
    };
  }

  /**
   * @inheritdoc
   * @description Converts a single `Message` into the format expected by the Ollama API.
   * @protected
   */
  protected convertSingleMessage(message: Message): unknown {
    return this.convertMessage(message);
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
