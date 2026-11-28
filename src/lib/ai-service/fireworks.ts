import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';
import { Message } from '@/models/chat';
import { createLlmFetch } from './desktop-fetch';
import {
  supportsToolsForModel,
  estimateContextWindowForModel,
} from './model-capability-rules';

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
      fetch: createLlmFetch(),
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.Fireworks`.
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.Fireworks;
  }

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Note: we no longer strip thinking/thinkingSignature fields. Unknown fields
    // are silently dropped by the API, so this is safe for all providers.
    return message;
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    return supportsToolsForModel(AIServiceProvider.Fireworks, modelName);
  }

  static estimateContextWindowForModel(modelName: string): number {
    return estimateContextWindowForModel(
      AIServiceProvider.Fireworks,
      modelName,
    );
  }

  /**
   * @inheritdoc
   */
  supportsTools(modelName: string): boolean {
    return FireworksService.supportsToolsForModel(modelName);
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    return FireworksService.estimateContextWindowForModel(modelName);
  }
}
