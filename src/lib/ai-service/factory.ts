import { getLogger } from '../logger';
import { AIServiceProvider, AIServiceConfig, IAIService } from './types';
import { GroqService } from './groq';
import { OpenAIService } from './openai';
import { AnthropicService } from './anthropic';
import { GeminiService } from './gemini';
import { FireworksService } from './fireworks';
import { CerebrasService } from './cerebras';
import { EmptyAIService } from './empty';

const logger = getLogger('AIService');

// --- Enhanced Service Factory ---

interface ServiceInstance {
  service: IAIService;
  apiKey: string;
  created: number;
}

export class AIServiceFactory {
  private static instances: Map<string, ServiceInstance> = new Map();
  private static readonly INSTANCE_TTL = 1000 * 60 * 60; // 1 hour

  static getService(
    provider: AIServiceProvider,
    apiKey: string,
    config?: AIServiceConfig,
  ): IAIService {
    const instanceKey = `${provider}:${apiKey}`;
    const now = Date.now();

    // Clean up expired instances
    this.cleanupExpiredInstances(now);

    const existing = this.instances.get(instanceKey);
    if (existing && now - existing.created < this.INSTANCE_TTL) {
      return existing.service;
    }

    // Dispose of old instance if it exists
    if (existing) {
      existing.service.dispose();
      this.instances.delete(instanceKey);
    }

    let service: IAIService;
    try {
      switch (provider) {
        case AIServiceProvider.Groq:
          service = new GroqService(apiKey, config);
          break;
        case AIServiceProvider.OpenAI:
          service = new OpenAIService(apiKey, config);
          break;
        case AIServiceProvider.Anthropic:
          service = new AnthropicService(apiKey, config);
          break;
        case AIServiceProvider.Gemini:
          service = new GeminiService(apiKey, config);
          break;
        case AIServiceProvider.Fireworks:
          service = new FireworksService(apiKey, config);
          break;
        case AIServiceProvider.Cerebras:
          service = new CerebrasService(apiKey, config);
          break;
        default:
          logger.warn(
            `Unknown AI service provider: ${provider}. Returning EmptyAIService.`,
          );
          service = new EmptyAIService();
          break;
      }
    } catch (e) {
      logger.error(
        `Failed to create service for provider ${provider} with error: ${e}. Returning EmptyAIService.`,
      );
      service = new EmptyAIService();
    }

    this.instances.set(instanceKey, {
      service,
      apiKey,
      created: now,
    });

    return service;
  }

  static disposeAll(): void {
    for (const instance of this.instances.values()) {
      instance.service.dispose();
    }
    this.instances.clear();
  }

  private static cleanupExpiredInstances(now: number): void {
    for (const instanceKey of this.instances.keys()) {
      const instance = this.instances.get(instanceKey);
      if (instance && now - instance.created >= this.INSTANCE_TTL) {
        instance.service.dispose();
        this.instances.delete(instanceKey);
      }
    }
  }
}
