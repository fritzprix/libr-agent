import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';

/**
 * An AI service implementation for the Fireworks AI provider.
 * This service extends `OpenAIService` as the Fireworks API is compatible
 * with the OpenAI API, but it overrides the base URL to point to the
 * Fireworks endpoint.
 */
export class FireworksService extends OpenAIService {
  /**
   * Initializes a new instance of the `FireworksService`.
   * @param apiKey The Fireworks AI API key.
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      baseURL: 'https://api.fireworks.ai/inference/v1',
      dangerouslyAllowBrowser: true,
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Fireworks`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Fireworks;
  }
}
