import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';

export class FireworksService extends OpenAIService {
  constructor(apiKey: string, config?: AIServiceConfig) {
    super(apiKey, config);
    this.openai = new OpenAI({
      apiKey: this.apiKey,
      baseURL: 'https://api.fireworks.ai/inference/v1',
      dangerouslyAllowBrowser: true,
    });
  }

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Fireworks;
  }
}
