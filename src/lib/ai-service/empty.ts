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

  dispose(): void {
    // No-op
  }
}
