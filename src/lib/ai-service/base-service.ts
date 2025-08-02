import { Message } from '@/models/chat';
import { MCPTool } from '../mcp-types';
import {
  AIServiceConfig,
  AIServiceProvider,
  AIServiceError,
  IAIService,
} from './types';
import { MessageValidator } from './validators';
import { withRetry, withTimeout } from '../retry-utils';

// --- Base Service Class with Common Functionality ---

export abstract class BaseAIService implements IAIService {
  protected defaultConfig: AIServiceConfig = {
    timeout: 30000,
    maxRetries: 3,
    retryDelay: 1000,
    maxTokens: 4096,
    temperature: 0.7,
  };

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
    messages.forEach(MessageValidator.validateMessage);
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

  abstract streamChat(
    messages: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      availableTools?: MCPTool[];
      config?: AIServiceConfig;
    },
  ): AsyncGenerator<string, void, void>;

  abstract getProvider(): AIServiceProvider;
  abstract dispose(): void;
}
