import { AIServiceProvider, AIServiceError } from './types';
import { BaseAIService } from './base-service';

export class EmptyAIService extends BaseAIService {
  constructor() {
    super('empty_api_key'); // Dummy API key
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Empty;
  }

  async *streamChat(): AsyncGenerator<string, void, void> {
    yield '';
    throw new AIServiceError(
      `EmptyAIService does not support streaming chat`,
      AIServiceProvider.Empty,
    );
    // Yield nothing, this is an empty service
  }

  // Implementation of abstract methods from BaseAIService
  protected createSystemMessage(systemPrompt: string): unknown {
    void systemPrompt;
    return null;
  }

  protected convertSingleMessage(message: unknown): unknown {
    void message;
    return null;
  }

  dispose(): void {
    // No-op
  }
}
