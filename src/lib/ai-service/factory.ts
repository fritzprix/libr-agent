import { getLogger } from '../logger';
import { AIServiceProvider, AIServiceConfig, IAIService } from './types';
import { GroqService } from './groq';
import { OpenAIService } from './openai';
import { AnthropicService } from './anthropic';
import { GeminiService } from './gemini';
import { FireworksService } from './fireworks';
import { CerebrasService } from './cerebras';
import { OllamaService } from './ollama';
import { EmptyAIService } from './empty';

const logger = getLogger('AIService');

/**
 * An internal interface to store a cached AI service instance along with its metadata.
 * @internal
 */
interface ServiceInstance {
  service: IAIService;
  apiKey: string;
  created: number;
}

/**
 * A factory class for creating and managing AI service instances.
 * It provides a centralized way to get service instances, caches them to avoid
 * re-instantiation, and handles their lifecycle (e.g., disposal of expired instances).
 */
export class AIServiceFactory {
  private static instances: Map<string, ServiceInstance> = new Map();
  private static readonly INSTANCE_TTL = 1000 * 60 * 60; // 1 hour

  /**
   * Gets an instance of an AI service for a given provider.
   * It uses a cached instance if a valid one exists, otherwise it creates a new one.
   *
   * @param provider The AI service provider to get an instance for.
   * @param apiKey The API key for the service.
   * @param config Optional configuration for the service.
   * @returns An instance of a class that implements the `IAIService` interface.
   *          Returns an `EmptyAIService` instance if the provider is unknown or creation fails.
   */
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
        case AIServiceProvider.Ollama:
          service = new OllamaService(apiKey, config);
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

  /**
   * Disposes of all cached service instances and clears the cache.
   */
  static disposeAll(): void {
    for (const instance of this.instances.values()) {
      instance.service.dispose();
    }
    this.instances.clear();
  }

  /**
   * Cleans up any cached service instances that have exceeded their time-to-live (TTL).
   * @param now The current timestamp (e.g., from `Date.now()`).
   * @private
   */
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
