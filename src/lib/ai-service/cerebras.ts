import Cerebras from '@cerebras/cerebras_cloud_sdk';
import type { ChatCompletion as CerebrasCompletion } from '@cerebras/cerebras_cloud_sdk/resources/chat/completions';
import { getLogger } from '../logger';
import { Message, ToolCall } from '@/models/chat';
import { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { ensureSchemaTypeField, formatToolResultForLlm } from './utils';

const logger = getLogger('CerebrasService');

// Constants
const DEFAULT_MODEL = 'llama3.1-8b';
const TOOL_CALL_TYPE = 'function' as const;

// Internal Interfaces
/** @internal */
interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
  signal?: AbortSignal;
}
/** @internal */
interface ChunkChoice {
  delta?: {
    content?: string;
    tool_calls?: ToolCall[];
  };
  finish_reason?: string;
}
/** @internal */
interface StreamingChunk {
  choices?: ChunkChoice[];
}
/** @internal */
interface StreamChunk {
  content?: string;
  tool_calls?: ToolCall[];
  error?: string;
}
/** @internal */
type CerebrasMessage =
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.SystemMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.UserMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.AssistantMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.ToolMessageRequest;

/**
 * An AI service implementation for interacting with Cerebras language models.
 */
export class CerebrasService extends BaseAIService<
  CerebrasMessage,
  Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool
> {
  private cerebras: Cerebras | null;

  /**
   * Initializes a new instance of the `CerebrasService`.
   * @param apiKey The Cerebras API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.cerebras = new Cerebras({
      apiKey: this.apiKey,
      maxRetries: config?.maxRetries ?? 2,
      timeout: config?.timeout ?? 60000,
    });
  }

  /**
   * Gets the provider identifier.
   * @returns `AIServiceProvider.Cerebras`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Cerebras;
  }

  /**
   * @inheritdoc
   */
  convertTools(
    mcpTools: MCPTool[],
  ): Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool[] {
    return mcpTools.map((tool) => ({
      type: 'function',
      function: {
        name: tool.name,
        description: tool.description || '',
        parameters: ensureSchemaTypeField(
          tool.inputSchema as unknown as Record<string, unknown>,
        ),
      },
    }));
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    const lowerName = modelName.toLowerCase();
    return lowerName.includes('llama3.1') || lowerName.includes('llama-3.1');
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('llama3.1-70b')) return 131072;
    if (lowerName.includes('llama3.1-8b')) return 131072;
    return 8192;
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Cerebras doesn't support special thinking fields yet
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
    return CerebrasService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return CerebrasService.estimateContextWindowForModel(modelName);
  }

  /**
   * Initiates a streaming chat session with the Cerebras API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat, including model name, system prompt, and tools.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
   */
  protected async *doStreamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    const { config, tools, sanitizedMessages } = this.prepareStreamChat(
      messages,
      options,
    );

    try {
      const cerebrasMessages = this.convertMessages(
        sanitizedMessages,
        options.systemPrompt,
      );
      const model = options.modelName || config.defaultModel || DEFAULT_MODEL;
      const abortSignal = options.signal;

      const stream = await this.withRetry(
        async (): Promise<AsyncIterable<unknown>> => {
          if (!this.cerebras) {
            throw new Error('Cerebras client not initialized');
          }

          return await this.cerebras.chat.completions.create(
            {
              messages: cerebrasMessages,
              model,
              stream: true,
              tools: tools,
              tool_choice: tools ? 'auto' : undefined,
            },
            { signal: abortSignal },
          );
        },
        abortSignal,
      );

      if (abortSignal?.aborted) {
        this.logger.info('Stream aborted before iteration');
        return;
      }

      yield* this.streamChatWithTTFT(
        (async function* (
          service: CerebrasService,
        ): AsyncGenerator<string, void, void> {
          for await (const chunk of stream) {
            if (abortSignal?.aborted) {
              service.logger.info('Stream aborted during iteration');
              break;
            }

            // Handle usage metrics usually found in the final chunk
            const chunkObj = chunk as unknown as Record<string, unknown>;
            if (chunkObj?.usage) {
              const u = chunkObj.usage as unknown as {
                prompt_tokens?: number;
                completion_tokens?: number;
                total_tokens?: number;
                prompt_tokens_details?: { cached_tokens?: number };
              };
              yield JSON.stringify({
                usage: {
                  promptTokens: u.prompt_tokens || 0,
                  completionTokens: u.completion_tokens || 0,
                  totalTokens: u.total_tokens || 0,
                  cachedPromptTokens: u.prompt_tokens_details?.cached_tokens,
                },
              });
            }

            const processedChunk = service.processChunk(chunk);
            if (processedChunk) {
              yield processedChunk;
            }
          }
        })(this),
      );
    } catch (error: unknown) {
      this.handleStreamingError(error, { messages, options, config });
    }
  }

  /**
   * Processes a single chunk from the streaming response.
   * @param chunk The raw chunk from the stream.
   * @returns A JSON string representing the processed chunk, or null if the chunk is empty.
   * @private
   */
  private processChunk(chunk: unknown): string | null {
    try {
      // Type guard for chunk structure
      if (!this.isValidStreamingChunk(chunk)) {
        return null;
      }

      const choices = chunk.choices;
      if (!choices || !Array.isArray(choices) || choices.length === 0) {
        return null;
      }

      const delta = choices[0]?.delta;
      if (!delta) {
        return null;
      }

      const response: StreamChunk = {};

      // Handle tool calls
      if (delta.tool_calls) {
        response.tool_calls = delta.tool_calls;
      }

      // Handle content
      if (delta.content) {
        response.content = delta.content;
      }

      // Only return if we have meaningful data
      if (response.content || response.tool_calls) {
        return JSON.stringify(response);
      }

      return null;
    } catch (error: unknown) {
      logger.error('Failed to process chunk', { error, chunk });
      return JSON.stringify({ error: 'Failed to process response chunk' });
    }
  }

  /**
   * A type guard to validate the structure of a streaming chunk.
   * @param chunk The chunk to validate.
   * @returns True if the chunk is a valid `StreamingChunk`, false otherwise.
   * @private
   */
  private isValidStreamingChunk(chunk: unknown): chunk is StreamingChunk {
    return (
      chunk != null &&
      typeof chunk === 'object' &&
      'choices' in chunk &&
      Array.isArray(chunk.choices)
    );
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the Cerebras API.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `CerebrasMessage` objects.
   * @private
   */
  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): CerebrasMessage[] {
    if (!Array.isArray(messages) || messages.length === 0) {
      throw new Error('Messages must be a non-empty array');
    }

    const cerebrasMessages: CerebrasMessage[] = [];

    // Add system prompt if provided
    if (systemPrompt?.trim()) {
      cerebrasMessages.push({
        role: 'system',
        content: systemPrompt.trim(),
      });
    }

    // Convert each message
    for (const message of messages) {
      const converted = this.convertMessage(message);
      if (converted) {
        cerebrasMessages.push(converted);
      }
    }

    return cerebrasMessages;
  }

  /**
   * Converts a single `Message` object to the corresponding `CerebrasMessage` format.
   * @param message The message to convert.
   * @returns A `CerebrasMessage` object, or null if the message is invalid.
   * @private
   */
  private convertMessage(message: Message): CerebrasMessage | null {
    if (!message?.role) {
      logger.warn('Invalid message structure', { message });
      return null;
    }

    switch (message.role) {
      case 'user':
        return this.convertUserMessage(message);

      case 'assistant':
        return this.convertAssistantMessage(message);

      case 'tool':
        return this.convertToolMessage(message);

      default:
        logger.warn(`Unsupported message role: ${message.role}`);
        return null;
    }
  }

  /**
   * Converts a user message.
   * @param message The user message to convert.
   * @returns A `CerebrasMessage` object, or null if invalid.
   * @private
   */
  private convertUserMessage(message: Message): CerebrasMessage | null {
    if (typeof message.content !== 'string') {
      logger.warn('User message content must be string');
      return null;
    }
    return {
      role: 'user',
      content: this.processMessageContent(message.content),
    };
  }

  /**
   * Converts an assistant message, handling both text content and tool calls.
   * @param message The assistant message to convert.
   * @returns A `CerebrasMessage` object, or null if invalid.
   * @private
   */
  private convertAssistantMessage(message: Message): CerebrasMessage | null {
    // Handle assistant message with tool calls
    if (
      message.tool_calls &&
      Array.isArray(message.tool_calls) &&
      message.tool_calls.length > 0
    ) {
      const validToolCalls = message.tool_calls.filter(
        (tc): tc is NonNullable<typeof tc> =>
          tc != null &&
          typeof tc === 'object' &&
          'id' in tc &&
          'function' in tc &&
          tc.function != null &&
          typeof tc.function === 'object' &&
          'name' in tc.function &&
          typeof tc.function.name === 'string',
      );

      if (validToolCalls.length === 0) {
        logger.warn('Assistant message has invalid tool calls');
        return null;
      }

      return {
        role: 'assistant',
        content: this.processMessageContent(message.content) || null,
        tool_calls: validToolCalls.map((tc) => ({
          id: tc.id as string,
          type: TOOL_CALL_TYPE,
          function: {
            name: tc.function.name,
            arguments:
              'arguments' in tc.function &&
              typeof tc.function.arguments === 'string'
                ? tc.function.arguments
                : '{}',
          },
        })),
      };
    }

    // Handle regular assistant message
    if (typeof message.content !== 'string') {
      logger.warn('Assistant message content must be string');
      return null;
    }

    return {
      role: 'assistant',
      content: this.processMessageContent(message.content),
    };
  }

  /**
   * Converts a tool message.
   * @param message The tool message to convert.
   * @returns A `CerebrasMessage` object, or null if invalid.
   * @private
   */
  private convertToolMessage(message: Message): CerebrasMessage | null {
    if (!message.tool_call_id) {
      logger.warn('Tool message missing tool_call_id');
      return null;
    }

    return {
      role: 'tool',
      tool_call_id: message.tool_call_id,
      content: formatToolResultForLlm(message),
    };
  }

  /**
   * Performs a non-streaming text generation request using the Cerebras API.
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
      signal?: AbortSignal;
    },
  ): Promise<SamplingResponse> {
    if (!this.cerebras) {
      throw new Error('CerebrasService has been disposed');
    }
    const config = this.mergeConfig(options);
    const model = options?.modelName || config.defaultModel || '';
    const s = options?.samplingOptions;

    const response = await this.withRetry(
      () =>
        this.cerebras!.chat.completions.create({
          model,
          stream: false,
          messages: [{ role: 'user', content: prompt }],
          max_tokens: s?.maxTokens ?? config.maxTokens,
          temperature: s?.temperature ?? config.temperature,
          top_p: s?.topP,
          presence_penalty: s?.presencePenalty,
          frequency_penalty: s?.frequencyPenalty,
          stop: s?.stopSequences,
        }),
      options?.signal,
    );

    // The Cerebras SDK's ChatCompletion union includes streaming chunks; narrow to the response type
    const completionResponse =
      response as CerebrasCompletion.ChatCompletionResponse;
    const choice = completionResponse.choices[0];
    const text = choice.message.content ?? '';
    const usage = completionResponse.usage;

    return {
      jsonrpc: '2.0',
      id: null,
      result: {
        content: [{ type: 'text', text }],
        sampling: {
          finishReason: choice.finish_reason === 'stop' ? 'stop' : 'length',
          usage: usage
            ? {
                promptTokens: usage.prompt_tokens ?? 0,
                completionTokens: usage.completion_tokens ?? 0,
                totalTokens: usage.total_tokens ?? 0,
              }
            : undefined,
          model: completionResponse.model,
        },
      },
    };
  }

  /**
   * @inheritdoc
   * @description Clears the reference to the Cerebras client to allow for garbage collection.
   */
  dispose(): void {
    // Clear reference to allow garbage collection
    this.cerebras = null;
  }
}
