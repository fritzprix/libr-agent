import { AIServiceProvider, AIServiceError } from './types';
import { MCPTool } from '@/lib/mcp';
import { Message } from '@/models/chat';
import { BaseAIService } from './base-service';

/**
 * A placeholder AI service that does nothing. It can be used for testing
 * or as a default when no other service is available.
 */
export class EmptyAIService extends BaseAIService<unknown, never> {
  /**
   * Initializes a new instance of the `EmptyAIService`.
   */
  constructor() {
    super('empty_api_key'); // Dummy API key
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Empty`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Empty;
  }

  /**
   * @inheritdoc
   * @throws {AIServiceError} Always throws as tool conversion is not supported.
   */
  convertTools(mcpTools: MCPTool[]): never[] {
    void mcpTools;
    throw new AIServiceError(
      'Tool conversion not supported for Empty AIServiceProvider',
      AIServiceProvider.Empty,
    );
  }

  /**
   * @inheritdoc
   * @description This implementation immediately throws an error as the empty service
   * does not support chat streaming.
   */
  protected async *doStreamChat(
    messages: Message[],
    options?: unknown,
  ): AsyncGenerator<string, void, void> {
    void messages;
    void options;
    if (this.getAbortSignal().aborted) {
      this.logger.info('EmptyAIService stream cancelled before starting.');
      return;
    }
    yield '';
    throw new AIServiceError(
      `EmptyAIService does not support streaming chat`,
      AIServiceProvider.Empty,
    );
    // Yield nothing, this is an empty service
  }

  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): unknown[] {
    void messages;
    void systemPrompt;
    return [];
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    return message;
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    void modelName;
    return false;
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    void modelName;
    return 0;
  }

  /**
   * @inheritdoc
   * @description This is a no-op as there are no resources to clean up.
   */
  dispose(): void {
    // No-op
  }
}
