import Cerebras from '@cerebras/cerebras_cloud_sdk';
import { getLogger } from '../logger';
import { Message, ToolCall } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToCerebrasTools } from './tool-converters';

const logger = getLogger('CerebrasService');

// Constants
const DEFAULT_MODEL = 'llama3.1-8b';
const TOOL_CALL_TYPE = 'function' as const;

// Internal Interfaces
/** @internal */
interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
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
export class CerebrasService extends BaseAIService {
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
   * Initiates a streaming chat session with the Cerebras API.
   * @param messages The array of messages for the conversation.
   * @param options Optional parameters for the chat, including model name, system prompt, and tools.
   * @yields A JSON string for each chunk of the response, containing content and/or tool calls.
   */
  async *streamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    const { config } = this.prepareStreamChat(messages, options);

    try {
      const cerebrasMessages = this.convertToCerebrasMessages(
        messages,
        options.systemPrompt,
      );
      const tools = this.prepareTools(options.availableTools);
      const model = options.modelName || config.defaultModel || DEFAULT_MODEL;

      logger.info('Cerebras API call:', {
        model,
        messagesCount: cerebrasMessages.length,
        hasTools: !!tools?.length,
      });

      const stream = await this.withRetry(
        async (): Promise<AsyncIterable<unknown>> => {
          if (!this.cerebras) {
            throw new Error('Cerebras client not initialized');
          }

          return await this.cerebras.chat.completions.create({
            messages: cerebrasMessages,
            model,
            stream: true,
            tools,
            tool_choice: tools ? 'auto' : undefined,
          });
        },
      );

      for await (const chunk of stream) {
        const processedChunk = this.processChunk(chunk);
        if (processedChunk) {
          yield processedChunk;
        }
      }
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
   * Converts an array of `MCPTool` objects to the format required by the Cerebras API.
   * @param availableTools The array of `MCPTool` objects.
   * @returns An array of Cerebras-compatible tool objects, or undefined if no tools are provided.
   * @private
   */
  private prepareTools(
    availableTools?: MCPTool[],
  ): Cerebras.Chat.Completions.ChatCompletionCreateParams.Tool[] | undefined {
    if (!availableTools?.length) {
      return undefined;
    }

    try {
      return convertMCPToolsToCerebrasTools(availableTools);
    } catch (error: unknown) {
      logger.error('Failed to convert tools', { error });
      return undefined;
    }
  }

  /**
   * Converts an array of standard `Message` objects into the format required by the Cerebras API.
   * @param messages The array of messages to convert.
   * @param systemPrompt An optional system prompt to prepend.
   * @returns An array of `CerebrasMessage` objects.
   * @private
   */
  private convertToCerebrasMessages(
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
      content: this.processMessageContent(message.content) || '',
    };
  }

  /**
   * @inheritdoc
   * @description Creates a Cerebras-compatible system message object.
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
   * @description Converts a single `Message` into the format expected by the Cerebras API.
   * @protected
   */
  protected convertSingleMessage(message: Message): unknown {
    return this.convertMessage(message);
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
