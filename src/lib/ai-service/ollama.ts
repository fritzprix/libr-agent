import ollama from 'ollama/browser';
import type { ListResponse, ModelResponse } from 'ollama';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { ModelInfo } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';

const logger = getLogger('AIService');

// Constants
const DEFAULT_MODEL = 'llama3.1';
const DEFAULT_HOST = 'http://127.0.0.1:11434';

// Interfaces
interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
}

interface SimpleOllamaMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
}

export class OllamaService extends BaseAIService {
  private host: string;

  constructor(apiKey: string, config?: AIServiceConfig & { host?: string }) {
    super(apiKey, config);
    this.host = config?.host || DEFAULT_HOST;
    logger.info(`Ollama service initialized with host: ${this.host}`);
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Ollama;
  }

  /**
   * Ollama 서버에서 실시간으로 모델 목록을 조회합니다.
   * ollama.list() API를 사용하여 서버에 설치된 모델들을 가져옵니다.
   */
  async listModels(): Promise<ModelInfo[]> {
    try {
      logger.info('Fetching models from Ollama server...');

      const response: ListResponse = await this.withRetry(async () => {
        return await ollama.list();
      });

      // ollama.list() 응답 구조에 맞춰 모델 정보 변환
      const models: ModelInfo[] = response.models.map(
        (model: ModelResponse) => ({
          id: model.name,
          name: model.name,
          contextWindow: 4096, // Ollama 기본값, 실제로는 모델마다 다름
          supportReasoning: true, // Ollama 모델들은 일반적으로 reasoning 지원
          supportTools: false, // Ollama의 tool calling은 아직 제한적
          supportStreaming: true, // Ollama는 스트리밍을 지원
          cost: { input: 0, output: 0 }, // Ollama는 로컬 실행이므로 비용 없음
          description: model.details?.family || model.name || `Ollama model`,
        }),
      );

      logger.info(`Found ${models.length} models on Ollama server`);
      return models;
    } catch (error) {
      logger.error('Failed to fetch models from Ollama server:', error);

      // 에러 발생시 빈 배열 반환 (서버가 꺼져있거나 연결 실패시)
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error';
      logger.warn(
        `Ollama server not available (${errorMessage}), returning empty model list`,
      );
      return [];
    }
  }

  async *streamChat(
    messages: Message[],
    options: StreamChatOptions = {},
  ): AsyncGenerator<string, void, void> {
    this.validateMessages(messages);

    const config = { ...this.defaultConfig, ...options.config };

    try {
      const ollamaMessages = this.convertToOllamaMessages(
        messages,
        options.systemPrompt,
      );
      const model = options.modelName || config.defaultModel || DEFAULT_MODEL;

      logger.info('Ollama API call:', {
        model,
        messagesCount: ollamaMessages.length,
        host: this.host,
      });

      const requestOptions = {
        model,
        messages: ollamaMessages,
        stream: true as const,
        keep_alive: '5m',
        options: {
          temperature: config.temperature || 0.7,
          num_predict: config.maxTokens || 4096,
        },
      };

      // Note: Tool support will be added in a future update
      // For now, we're focusing on basic chat functionality
      if (options.availableTools && options.availableTools.length > 0) {
        logger.warn('Tool calling not yet implemented for Ollama service');
      }

      const stream = await this.withRetry(async () => {
        return await ollama.chat(requestOptions);
      });

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
      // Type guard to check if chunk has the expected structure
      if (
        !chunk ||
        typeof chunk !== 'object' ||
        !('message' in chunk) ||
        !chunk.message ||
        typeof chunk.message !== 'object'
      ) {
        return null;
      }

      const message = chunk.message as { content?: string };

      const result: {
        content?: string;
        error?: string;
      } = {};

      // Handle content
      if (message.content && typeof message.content === 'string') {
        result.content = message.content;
      }

      // Only return if we have meaningful data
      if (result.content) {
        return JSON.stringify(result);
      }

      return null;
    } catch (error: unknown) {
      logger.error('Failed to process chunk', { error, chunk });
      return JSON.stringify({ error: 'Failed to process response chunk' });
    }
  }

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
          content: message.content || '',
        };

      case 'tool':
        // For now, convert tool messages to user messages with context
        return {
          role: 'user',
          content: `Tool result: ${message.content}`,
        };

      default:
        logger.warn(`Unsupported message role: ${message.role}`);
        return null;
    }
  }

  private convertUserMessage(message: Message): SimpleOllamaMessage | null {
    if (typeof message.content !== 'string') {
      logger.warn('User message content must be string');
      return null;
    }
    return { role: 'user', content: message.content };
  }

  private convertAssistantMessage(
    message: Message,
  ): SimpleOllamaMessage | null {
    // For now, just handle basic assistant messages
    // Tool calls will be handled in a future update
    if (message.tool_calls && message.tool_calls.length > 0) {
      logger.info('Converting tool calls to text for Ollama compatibility');
      const toolCallsText = message.tool_calls
        .map((tc) => `Called function: ${tc.function.name}`)
        .join(', ');
      return {
        role: 'assistant',
        content: message.content
          ? `${message.content}\n\n[${toolCallsText}]`
          : `[${toolCallsText}]`,
      };
    }

    if (typeof message.content !== 'string') {
      logger.warn('Assistant message content must be string');
      return null;
    }

    return { role: 'assistant', content: message.content };
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
        host: this.host,
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
    // Ollama client doesn't require explicit cleanup
    logger.info('Ollama service disposed');
  }
}
