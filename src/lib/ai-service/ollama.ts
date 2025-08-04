import { Ollama } from 'ollama/browser';
import type { ChatRequest, ListResponse, ModelResponse, Tool, Message as OllamaMessage } from 'ollama';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import { ModelInfo } from '../llm-config-manager';
import { AIServiceProvider, AIServiceConfig, AIServiceError } from './types';
import { BaseAIService } from './base-service';

const logger = getLogger('OllamaService');

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
  tool_calls?: Array<{
    id: string;
    type: 'function';
    function: {
      name: string;
      arguments: string;
    };
  }>;
  tool_call_id?: string;
}

// MCPTool을 Ollama Tool로 변환하는 함수
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

export class OllamaService extends BaseAIService {
  private host: string;
  private ollamaClient: Ollama;

  constructor(apiKey: string, config?: AIServiceConfig & { host?: string }) {
    super(apiKey, config);
    this.host = config?.host || DEFAULT_HOST;
    
    // Ollama 클라이언트 인스턴스 생성
    this.ollamaClient = new Ollama({ 
      host: this.host,
      headers: {
        'User-Agent': 'SynapticFlow/1.0',
      },
    });
    
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
        return await this.ollamaClient.list();
      });

      // ollama.list() 응답 구조에 맞춰 모델 정보 변환
      const models: ModelInfo[] = response.models.map(
        (model: ModelResponse) => ({
          id: model.name,
          name: model.name,
          contextWindow: this.getModelContextWindow(model.name),
          supportReasoning: true,
          supportTools: this.getModelToolSupport(model.name),
          supportStreaming: true,
          cost: { input: 0, output: 0 },
          description: model.details?.family || model.name || 'Ollama model',
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
      const ollamaTools = convertMCPToolsToOllamaTools(options.availableTools);

      logger.info('Ollama API call:', {
        model,
        messagesCount: ollamaMessages.length,
        host: this.host,
        toolsCount: ollamaTools.length,
      });

      const requestOptions: ChatRequest & { stream: true } = {
        model,
        messages: ollamaMessages as OllamaMessage[],
        stream: true,
        think: true,
        tools: ollamaTools,
        keep_alive: '5m',
        options: {
          temperature: config.temperature || 0.7,
          num_predict: config.maxTokens || 4096,
        },
      };

      const stream = await this.withRetry(async () => {
        return await this.ollamaClient.chat(requestOptions);
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

      const message = chunk.message as {
        content?: string;
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
      }

      // Handle tool calls
      if (message.tool_calls && Array.isArray(message.tool_calls)) {
        result.tool_calls = message.tool_calls.map((tc) => ({
          id: tc.id || `call_${Math.random().toString(36).substring(2, 15)}`,
          type: 'function' as const,
          function: {
            name: tc.function.name,
            arguments: typeof tc.function.arguments === 'string' 
              ? tc.function.arguments 
              : JSON.stringify(tc.function.arguments || {}),
          },
        }));
        logger.debug('Tool calls detected in chunk:', result.tool_calls);
      }

      // Return if we have meaningful data
      if (result.content || result.tool_calls) {
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
        // Ollama에서 tool 결과를 처리하기 위해 user 메시지로 변환
        return {
          role: 'user',
          content: `Tool result: ${message.content}`,
          tool_call_id: message.tool_call_id,
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
    const result: SimpleOllamaMessage = {
      role: 'assistant',
      content: message.content || '',
    };

    // Handle tool calls
    if (message.tool_calls && message.tool_calls.length > 0) {
      result.tool_calls = message.tool_calls.map((tc) => ({
        id: tc.id || this.generateToolCallId(),
        type: 'function' as const,
        function: {
          name: tc.function.name,
          arguments: typeof tc.function.arguments === 'string' 
            ? tc.function.arguments 
            : JSON.stringify(tc.function.arguments || {}),
        },
      }));
      logger.debug('Converted tool calls for assistant message', result.tool_calls);
    }

    return result;
  }

  private generateToolCallId(): string {
    return `call_${Math.random().toString(36).substring(2, 15)}`;
  }

  private getModelContextWindow(modelName: string): number {
    // 일반적인 Ollama 모델들의 컨텍스트 윈도우
    if (modelName.includes('llama3.1')) return 128000;
    if (modelName.includes('llama3')) return 8192;
    if (modelName.includes('llama2')) return 4096;
    if (modelName.includes('codellama')) return 16384;
    if (modelName.includes('mistral')) return 8192;
    if (modelName.includes('qwen')) return 32768;
    return 4096; // 기본값
  }

  private getModelToolSupport(modelName: string): boolean {
    // Tool calling을 지원하는 모델들 (실제 지원 여부는 모델마다 다름)
    const toolSupportModels = [
      'llama3.1',
      'llama3.2',
      'qwen',
      'mistral',
      'dolphin',
    ];
    return toolSupportModels.some(model => modelName.includes(model));
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
