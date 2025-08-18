import Cerebras from '@cerebras/cerebras_cloud_sdk';
import { getLogger } from '../logger';
import { Message, ToolCall } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';
import { convertMCPToolsToCerebrasTools } from './tool-converters';

const logger = getLogger('AIService');

// Constants
const DEFAULT_MODEL = 'llama3.1-8b';
const TOOL_CALL_TYPE = 'function' as const;

// Interfaces
interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
}

interface ChunkChoice {
  delta?: {
    content?: string;
    tool_calls?: ToolCall[];
  };
  finish_reason?: string;
}

interface StreamingChunk {
  choices?: ChunkChoice[];
}

interface StreamChunk {
  content?: string;
  tool_calls?: ToolCall[];
  error?: string;
}

type CerebrasMessage =
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.SystemMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.UserMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.AssistantMessageRequest
  | Cerebras.Chat.Completions.ChatCompletionCreateParams.ToolMessageRequest;

export class CerebrasService extends BaseAIService {
  private cerebras: Cerebras | null;

  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.cerebras = new Cerebras({
      apiKey: this.apiKey,
      maxRetries: config?.maxRetries ?? 2,
      timeout: config?.timeout ?? 60000,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Cerebras;
  }

  async *streamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    this.validateMessages(messages);

    const config = { ...this.defaultConfig, ...options.config };

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
      this.handleStreamError(error, messages, options, config);
    }
  }

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

  private isValidStreamingChunk(chunk: unknown): chunk is StreamingChunk {
    return (
      chunk != null &&
      typeof chunk === 'object' &&
      'choices' in chunk &&
      Array.isArray(chunk.choices)
    );
  }

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

  private convertUserMessage(message: Message): CerebrasMessage | null {
    if (typeof message.content !== 'string') {
      logger.warn('User message content must be string');
      return null;
    }
    return { role: 'user', content: this.processMessageContent(message.content) };
  }

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

    return { role: 'assistant', content: this.processMessageContent(message.content) };
  }

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

  private handleStreamError(
    error: unknown,
    messages: Message[],
    options: StreamChatOptions,
    config: AIServiceConfig,
  ): never {
    const serviceProvider = this.getProvider();
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    const errorStack = error instanceof Error ? error.stack : undefined;

    logger.error(`${serviceProvider} streaming failed`, {
      error: errorMessage,
      stack: errorStack,
      requestData: {
        model: options.modelName || config.defaultModel || DEFAULT_MODEL,
        messagesCount: messages.length,
        hasTools: !!options.availableTools?.length,
        systemPrompt: !!options.systemPrompt,
      },
    });

    throw new AIServiceError(
      `${serviceProvider} streaming failed: ${errorMessage}`,
      serviceProvider,
      undefined,
      error instanceof Error ? error : undefined,
    );
  }

  dispose(): void {
    // Clear reference to allow garbage collection
    this.cerebras = null;
  }
}
