import { Ollama } from 'ollama/browser';
import type {
  ChatRequest,
  ListResponse,
  ModelResponse,
  Tool,
  Message as OllamaMessage,
} from 'ollama';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { ModelInfo } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { supportsThinking, getContextWindow } from './model-capabilities';

const logger = getLogger('OllamaService');

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
/** @internal */
interface SimpleOllamaMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  tool_calls?: Array<{
    id: string;
    type: 'function';
    function: {
      name: string;
      arguments: Record<string, unknown>;
    };
  }>;
  tool_call_id?: string;
}

/**
 * Converts an array of `MCPTool` objects to the format required by the Ollama API.
 * @param mcpTools The array of `MCPTool` objects to convert.
 * @returns An array of Ollama-compatible `Tool` objects.
 * @internal
 */
function convertMCPToolsToOllamaTools(mcpTools?: MCPTool[]): Tool[] {
  if (!mcpTools || mcpTools.length === 0) {
    return [];
  }

  return mcpTools.map((tool) => ({
    type: 'function',
    function: {
      name: tool.name,
      description: tool.description,
      parameters: tool.inputSchema || {
        type: 'object',
        properties: {},
      },
    },
  }));
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
      const ollamaTools = convertMCPToolsToOllamaTools(options.availableTools);

      // Prepare reasoning parameter based on config
      // Ollama expects: think: true | 'low' | 'medium' | 'high'
      // Note: Not all models support granular levels ('low', 'medium', 'high')
      // For models that only support boolean (like qwen3), use true
      let thinkParam: boolean | 'low' | 'medium' | 'high' | undefined;
      if (config.enableReasoning) {
        // Check if model actually supports thinking (optional, for better UX)
        const modelSupportsThinking = await supportsThinking(
          model,
          AIServiceProvider.Ollama,
          { apiBase: this.host },
        );

        if (modelSupportsThinking) {
          // If reasoningEffort is specified, use it; otherwise default to true
          thinkParam = config.reasoningEffort || true;
          logger.info('Ollama thinking mode enabled', {
            model,
            thinkParam,
            detectedViaAPI: modelSupportsThinking,
          });
        } else {
          logger.debug(
            'Model may not support thinking, but will try anyway (Ollama ignores unsupported params)',
            { model },
          );
          // Still send the parameter - Ollama will ignore if not supported
          thinkParam = config.reasoningEffort || true;
        }
      }

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
    try {
      // Type guard to check if chunk has the expected structure
      if (
        !chunk ||
        typeof chunk !== 'object' ||
        !('message' in chunk) ||
        !chunk.message ||
        typeof chunk.message !== 'object'
      ) {
        logger.debug('Chunk missing expected structure, skipping', {
          hasChunk: !!chunk,
          chunkType: typeof chunk,
          hasMessage: chunk && typeof chunk === 'object' && 'message' in chunk,
          chunkKeys: chunk ? Object.keys(chunk) : [],
        });
        return null;
      }

      const message = chunk.message as {
        content?: string;
        thinking?: string; // Ollama reasoning mode thinking content
        tool_calls?: Array<{
          id: string;
          type: string;
          function: {
            name: string;
            arguments: string;
          };
        }>;
      };

      const result: {
        content?: string;
        thinking?: string; // Add thinking field to result
        tool_calls?: Array<{
          id: string;
          type: string;
          function: {
            name: string;
            arguments: string;
          };
        }>;
        error?: string;
      } = {};

      // Handle content
      if (message.content && typeof message.content === 'string') {
        result.content = message.content;
        logger.debug('Content extracted from chunk', {
          contentLength: message.content.length,
          contentPreview: message.content.substring(0, 100),
        });
      } else {
        logger.debug('No content in message', {
          hasContent: !!message.content,
          contentType: typeof message.content,
        });
      }

      // Handle thinking (Ollama reasoning mode)
      if (message.thinking && typeof message.thinking === 'string') {
        result.thinking = message.thinking;
        logger.debug('Thinking extracted from chunk', {
          thinkingLength: message.thinking.length,
          thinkingPreview: message.thinking.substring(0, 100),
        });
      }

      // Handle tool calls
      if (message.tool_calls && Array.isArray(message.tool_calls)) {
        result.tool_calls = message.tool_calls.map((tc) => ({
          id: tc.id || `call_${Math.random().toString(36).substring(2, 15)}`,
          type: 'function' as const,
          function: {
            name: tc.function.name,
            arguments:
              typeof tc.function.arguments === 'string'
                ? tc.function.arguments
                : JSON.stringify(tc.function.arguments || {}),
          },
        }));
        logger.debug('Tool calls detected in chunk:', {
          toolCallCount: result.tool_calls.length,
          toolCalls: result.tool_calls,
        });
      }

      // Return if we have meaningful data (content, thinking, or tool_calls)
      if (result.content || result.thinking || result.tool_calls) {
        return JSON.stringify(result);
      }

      logger.debug(
        'Chunk has no content, thinking, or tool_calls, returning null',
      );
      return null;
    } catch (error: unknown) {
      logger.error('Failed to process chunk', { error, chunk });
      return JSON.stringify({ error: 'Failed to process response chunk' });
    }
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
    if (!Array.isArray(messages) || messages.length === 0) {
      throw new Error('Messages must be a non-empty array');
    }

    const ollamaMessages: SimpleOllamaMessage[] = [];

    // Add system prompt if provided
    if (systemPrompt?.trim()) {
      ollamaMessages.push({
        role: 'system',
        content: systemPrompt.trim(),
      });
    }

    // Convert each message
    for (const message of messages) {
      const converted = this.convertMessage(message);
      if (converted) {
        ollamaMessages.push(converted);
      }
    }

    return ollamaMessages;
  }

  /**
   * Converts a single `Message` object to the corresponding `SimpleOllamaMessage` format.
   * @param message The message to convert.
   * @returns A `SimpleOllamaMessage` object, or null if the message is invalid.
   * @private
   */
  private convertMessage(message: Message): SimpleOllamaMessage | null {
    if (!message?.role) {
      logger.warn('Invalid message structure', { message });
      return null;
    }

    switch (message.role) {
      case 'user':
        return this.convertUserMessage(message);

      case 'assistant':
        return this.convertAssistantMessage(message);

      case 'system':
        // System messages are handled separately in convertToOllamaMessages
        return {
          role: 'system',
          content: this.processMessageContent(message.content) || '',
          tool_call_id: message.tool_call_id,
        };

      case 'tool':
        // Convert tool result to a user message for processing in Ollama
        return {
          role: 'user',
          content: `Tool result: ${this.processMessageContent(message.content)}`,
          tool_call_id: message.tool_call_id,
        };

      default:
        logger.warn(`Unsupported message role: ${message.role}`);
        return null;
    }
  }

  /**
   * Converts a user message to the Ollama format.
   * @param message The user message.
   * @returns A `SimpleOllamaMessage` object.
   * @private
   */
  private convertUserMessage(message: Message): SimpleOllamaMessage | null {
    return {
      role: 'user',
      content: this.processMessageContent(message.content),
    };
  }

  /**
   * Converts an assistant message to the Ollama format, including tool calls.
   * @param message The assistant message.
   * @returns A `SimpleOllamaMessage` object.
   * @private
   */
  private convertAssistantMessage(
    message: Message,
  ): SimpleOllamaMessage | null {
    const result: SimpleOllamaMessage = {
      role: 'assistant',
      content: this.processMessageContent(message.content) || '',
    };

    // Handle tool calls
    if (message.tool_calls && message.tool_calls.length > 0) {
      result.tool_calls = message.tool_calls.map((tc) => ({
        id: tc.id || this.generateToolCallId(),
        type: 'function' as const,
        function: {
          name: tc.function.name,
          arguments: this.parseArguments(tc.function.arguments),
        },
      }));
      logger.debug(
        'Converted tool calls for assistant message',
        result.tool_calls,
      );
    }

    return result;
  }

  /**
   * Generates a random ID for a tool call.
   * @returns A unique tool call ID string.
   * @private
   */
  private generateToolCallId(): string {
    return `call_${Math.random().toString(36).substring(2, 15)}`;
  }

  /**
   * Safely parses a string of tool call arguments into a record.
   * @param args The stringified arguments.
   * @returns A record of the arguments, or an empty object on failure.
   * @private
   */
  private parseArguments(args: string): Record<string, unknown> {
    try {
      if (typeof args === 'string') {
        return JSON.parse(args || '{}');
      }
      return args || {};
    } catch (error) {
      logger.warn('Failed to parse tool call arguments, using empty object:', {
        args,
        error,
      });
      return {};
    }
  }

  /**
   * Checks if a given Ollama model likely supports tool use based on its name.
   * @param modelName The name of the model.
   * @returns True if the model is known to support tools, false otherwise.
   * @private
   */
  private getModelToolSupport(modelName: string): boolean {
    // Models that support tool calling (actual support may vary by model)
    const toolSupportModels = [
      'llama3.1',
      'llama3.2',
      'qwen',
      'mistral',
      'dolphin',
    ];
    return toolSupportModels.some((model) => modelName.includes(model));
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
