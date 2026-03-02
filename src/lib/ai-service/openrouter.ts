import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';

/**
 * An AI service implementation for OpenRouter.
 * OpenRouter provides a unified OpenAI-compatible API that routes requests to
 * 200+ models from providers like Anthropic, Google, Meta, Mistral, and more.
 *
 * @see https://openrouter.ai/docs
 */
export class OpenRouterService extends OpenAIService {
  /**
   * Initializes a new instance of the `OpenRouterService`.
   * @param apiKey The OpenRouter API key (starts with "sk-or-").
   * @param config Optional configuration for the service.
   */
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      baseURL: 'https://openrouter.ai/api/v1',
      dangerouslyAllowBrowser: true,
      defaultHeaders: {
        // OpenRouter recommends these headers for request attribution
        'HTTP-Referer': 'https://github.com/fritzprix/libr-agent',
        'X-Title': 'LibrAgent',
      },
    });
  }

  /**
   * @inheritdoc
   * @returns `AIServiceProvider.OpenRouter`
   */
  getProvider(): AIServiceProvider {
    return AIServiceProvider.OpenRouter;
  }
}
