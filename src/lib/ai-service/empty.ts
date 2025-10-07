import { AIServiceProvider, AIServiceError } from './types';
import { BaseAIService } from './base-service';

/**
 * A placeholder AI service that does nothing. It can be used for testing
 * or as a default when no other service is available.
 */
export class EmptyAIService extends BaseAIService {
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
   * @description This implementation immediately throws an error as the empty service
   * does not support chat streaming.
   */
  async *streamChat(): AsyncGenerator<string, void, void> {
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

  /**
   * @inheritdoc
   * @description Returns null as there is no message conversion.
   * @protected
   */
  protected createSystemMessage(systemPrompt: string): unknown {
    void systemPrompt;
    return null;
  }

  /**
   * @inheritdoc
   * @description Returns null as there is no message conversion.
   * @protected
   */
  protected convertSingleMessage(message: unknown): unknown {
    void message;
    return null;
  }

  /**
   * @inheritdoc
   * @description This is a no-op as there are no resources to clean up.
   */
  dispose(): void {
    // No-op
  }
}
