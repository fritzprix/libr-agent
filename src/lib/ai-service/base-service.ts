import { Message } from '@/models/chat';
import {
  MCPTool,
  MCPContent,
  SamplingOptions,
  SamplingResponse,
} from '../mcp-types';
import {
  AIServiceConfig,
  AIServiceProvider,
  AIServiceError,
  IAIService,
} from './types';
import { ModelInfo, llmConfigManager } from '../llm-config-manager';
import { withRetry, withTimeout } from '../retry-utils';
import { convertMCPToolsToProviderTools } from './tool-converters';
import { MessageNormalizer } from './message-normalizer';
import { getLogger } from '../logger';

// --- Base Service Class with Common Functionality ---

export abstract class BaseAIService implements IAIService {
  protected defaultConfig: AIServiceConfig = {
    timeout: 30000,
    maxRetries: 3,
    retryDelay: 1000,
    maxTokens: 4096,
    temperature: 0.7,
  };

  protected logger = getLogger('BaseAIService');

  constructor(
    protected apiKey: string,
    protected config?: AIServiceConfig,
  ) {
    this.validateApiKey(apiKey);
    this.defaultConfig = { ...this.defaultConfig, ...config };
  }

  protected validateApiKey(apiKey: string): void {
    if (!apiKey || typeof apiKey !== 'string' || apiKey.trim().length === 0) {
      throw new AIServiceError('Invalid API key provided', this.getProvider());
    }
  }

  protected validateMessages(messages: Message[]): void {
    if (!Array.isArray(messages) || messages.length === 0) {
      throw new AIServiceError(
        'Messages array cannot be empty',
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

  protected async withRetry<T>(
    operation: () => Promise<T>,
    maxRetries: number = this.defaultConfig.maxRetries!,
  ): Promise<T> {
    try {
      return await withRetry(operation, {
        maxRetries,
        baseDelay: this.defaultConfig.retryDelay!,
        timeout: this.defaultConfig.timeout!,
        exponentialBackoff: true,
      });
    } catch (error) {
      throw new AIServiceError(
        (error as Error).message,
        this.getProvider(),
        undefined,
        error as Error,
      );
    }
  }

  protected async withTimeout<T>(
    promise: Promise<T>,
    timeoutMs: number,
  ): Promise<T> {
    return withTimeout(promise, timeoutMs);
  }

  /**
   * MCPContent 배열을 LLM용 텍스트로 변환
   */
  protected processMessageContent(content: MCPContent[]): string {
    // MCPContent 배열에서 텍스트만 추출
    return content
      .filter((item) => item.type === 'text')
      .map((item) => (item as { text: string }).text)
      .join('\n');
  }

  /**
   * MLM용 - 이미지 content도 처리
   */
  protected processMultiModalContent(
    content: MCPContent[],
  ): Array<{ type: string; text?: string; image?: string }> {
    return content.map((item) => {
      switch (item.type) {
        case 'text':
          return { type: 'text', text: (item as { text: string }).text };
        case 'image':
          return {
            type: 'image',
            image:
              (
                item as {
                  data?: string;
                  source?: { data?: string; uri?: string };
                }
              ).data ||
              (item as { source?: { data?: string; uri?: string } }).source
                ?.data ||
              (item as { source?: { data?: string; uri?: string } }).source
                ?.uri,
          };
        default:
          return { type: 'text', text: `[${item.type}]` };
      }
    });
  }

  /**
   * Common error handling helper for streaming operations
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
      };
      config: AIServiceConfig;
    },
  ): never {
    const serviceProvider = this.getProvider();
    const errorMessage =
      error instanceof Error ? error.message : 'Unknown error';
    const errorStack = error instanceof Error ? error.stack : undefined;

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
   * Configuration merge helper
   */
  protected mergeConfig(options?: {
    config?: AIServiceConfig;
  }): AIServiceConfig {
    return { ...this.defaultConfig, ...options?.config };
  }

  /**
   * streamChat preprocessing common logic
   */
  protected prepareStreamChat(
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    } = {},
  ): {
    config: AIServiceConfig;
    tools?: unknown[];
    sanitizedMessages: Message[];
  } {
    this.validateMessages(messages);
    const config = this.mergeConfig(options);

    const tools = options.availableTools
      ? convertMCPToolsToProviderTools(
          options.availableTools,
          this.getProvider(),
        )
      : undefined;

    // Apply vendor-specific message sanitization
    const sanitizedMessages = this.sanitizeMessages(messages);

    return { config, tools, sanitizedMessages };
  }

  /**
   * Sanitize messages for vendor-specific compatibility
   * Base implementation uses MessageNormalizer, but services can override
   */
  protected sanitizeMessages(messages: Message[]): Message[] {
    return MessageNormalizer.sanitizeMessagesForProvider(
      messages,
      this.getProvider(),
    );
  }

  /**
   * Basic message conversion template method
   */
  protected convertMessagesTemplate(
    messages: Message[],
    systemPrompt?: string,
  ): unknown[] {
    const result: unknown[] = [];

    if (systemPrompt) {
      const systemMessage = this.createSystemMessage(systemPrompt);
      if (systemMessage) {
        result.push(systemMessage);
      }
    }

    for (const message of messages) {
      const converted = this.convertSingleMessage(message);
      if (converted) {
        result.push(converted);
      }
    }

    return result;
  }

  /**
   * Default implementation returns models from llmConfigManager for the provider.
   * Override this method for services that need dynamic model discovery (e.g., Ollama).
   */
  async listModels(): Promise<ModelInfo[]> {
    const provider = this.getProvider();
    const models = llmConfigManager.getModelsForProvider(provider);

    if (!models) {
      return [];
    }

    // Record<string, ModelInfo>를 ModelInfo[]로 변환
    return Object.values(models);
  }

  // Provider-specific abstract methods that must be implemented by subclasses
  protected abstract createSystemMessage(systemPrompt: string): unknown;
  protected abstract convertSingleMessage(message: Message): unknown;

  abstract streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  /**
   * Default implementation of sampleText - can be overridden by specific services
   */
  async sampleText(
    prompt: string,
    options?: {
      modelName?: string;
      samplingOptions?: SamplingOptions;
      config?: AIServiceConfig;
    },
  ): Promise<SamplingResponse> {
    void prompt;
    void options;
    throw new AIServiceError(
      'sampleText not implemented for this service',
      this.getProvider(),
    );
  }

  abstract getProvider(): AIServiceProvider;
  abstract dispose(): void;
}
