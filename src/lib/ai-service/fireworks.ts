import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';
import { Message } from '@/models/chat';

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

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // Fireworks models generally don't support Anthropic's thinking fields
    if (message.thinking) {
      delete message.thinking;
    }
    if (message.thinkingSignature) {
      delete message.thinkingSignature;
    }
    return message;
  }

  /**
   * @inheritdoc
   */
  static supportsToolsForModel(modelName: string): boolean {
    void modelName;
    return true;
  }

  static estimateContextWindowForModel(modelName: string): number {
    const lowerName = modelName.toLowerCase();
    if (lowerName.includes('llama-3.1-405b')) return 128000;
    if (lowerName.includes('qwen')) return 32768;
    return 128000;
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
