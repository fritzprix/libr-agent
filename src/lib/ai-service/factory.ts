import { getLogger } from '../logger';
import { AIServiceProvider, AIServiceConfig, IAIService } from './types';
import { GroqService } from './groq';
import { OpenAIService } from './openai';
import { AnthropicService } from './anthropic';
import { GeminiService } from './gemini';
import { FireworksService } from './fireworks';
import { CerebrasService } from './cerebras';
import { OllamaService } from './ollama';
import { OpenRouterService } from './openrouter';
import { EmptyAIService } from './empty';
import { LLMConfigManager } from '../llm-config-manager';
import { registerAIServiceFactory } from './model-capabilities';

const logger = getLogger('AIService');
const configManager = new LLMConfigManager();

/**
 * An internal interface to store a cached AI service instance along with its metadata.
 * @internal
 */
interface ServiceInstance {
  service: IAIService;
  apiKey: string;
  created: number;
}

interface CapabilityDelegate {
  supportsTools(modelName: string): boolean;
  estimateContextWindow(modelName: string): number;
}

function buildConfigCacheKey(config?: AIServiceConfig): string {
  if (!config) {
    return '';
  }

  // Only include constructor-time options that materially change which backend
  // endpoint/client instance this service should talk to, plus retry policy so
  // settings.advanced.maxRetries / retryDelay recreate the service.
  return JSON.stringify({
    baseUrl: config.baseUrl ?? '',
    use3rdParty: Boolean(config.use3rdParty),
    customModelId: config.customModelId ?? '',
    maxRetries: config.maxRetries ?? null,
    retryDelay: config.retryDelay ?? null,
  });
}

/**
 * A factory class for creating and managing AI service instances.
 * It provides a centralized way to get service instances, caches them to avoid
 * re-instantiation, and handles their lifecycle (e.g., disposal of expired instances).
 */
export class AIServiceFactory {
  private static instances: Map<string, ServiceInstance> = new Map();
  private static readonly INSTANCE_TTL = 1000 * 60 * 60; // 1 hour

  private static computeEffectiveApiKey(
    provider: AIServiceProvider,
    apiKey: string,
  ): string {
    const providers = configManager.getProviders();
    const providerInfo = providers[provider];
    const requiresApiKey = providerInfo?.requiresApiKey ?? true;

    return !requiresApiKey && (!apiKey || apiKey.trim().length === 0)
      ? `${provider}-local`
      : apiKey;
  }

  private static buildInstanceKey(
    provider: AIServiceProvider,
    apiKey: string,
    config?: AIServiceConfig,
  ): string {
    const effectiveApiKey = this.computeEffectiveApiKey(provider, apiKey);
    return `${provider}:${effectiveApiKey}:${buildConfigCacheKey(config)}`;
  }

  static getCapabilityDelegate(
    provider: AIServiceProvider,
  ): CapabilityDelegate {
    switch (provider) {
      case AIServiceProvider.Groq:
        return {
          supportsTools: GroqService.supportsToolsForModel,
          estimateContextWindow: GroqService.estimateContextWindowForModel,
        };
      case AIServiceProvider.OpenAI:
        return {
          supportsTools: OpenAIService.supportsToolsForModel,
          estimateContextWindow: OpenAIService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Anthropic:
        return {
          supportsTools: AnthropicService.supportsToolsForModel,
          estimateContextWindow: AnthropicService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Gemini:
        return {
          supportsTools: GeminiService.supportsToolsForModel,
          estimateContextWindow: GeminiService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Fireworks:
        return {
          supportsTools: FireworksService.supportsToolsForModel,
          estimateContextWindow: FireworksService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Cerebras:
        return {
          supportsTools: CerebrasService.supportsToolsForModel,
          estimateContextWindow: CerebrasService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Ollama:
        return {
          supportsTools: OllamaService.supportsToolsForModel,
          estimateContextWindow: OllamaService.estimateContextWindowForModel,
        };
      case AIServiceProvider.OpenRouter:
        return {
          supportsTools: OpenRouterService.supportsToolsForModel,
          estimateContextWindow:
            OpenRouterService.estimateContextWindowForModel,
        };
      case AIServiceProvider.Empty:
      default:
        return {
          supportsTools: EmptyAIService.supportsToolsForModel,
          estimateContextWindow: EmptyAIService.estimateContextWindowForModel,
        };
    }
  }

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
    const effectiveApiKey = this.computeEffectiveApiKey(provider, apiKey);
    const instanceKey = this.buildInstanceKey(provider, apiKey, config);
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
          service = new GroqService(effectiveApiKey, config);
          break;
        case AIServiceProvider.OpenAI:
          service = new OpenAIService(effectiveApiKey, config);
          break;
        case AIServiceProvider.Anthropic:
          service = new AnthropicService(effectiveApiKey, config);
          break;
        case AIServiceProvider.Gemini:
          service = new GeminiService(effectiveApiKey, config);
          break;
        case AIServiceProvider.Fireworks:
          service = new FireworksService(effectiveApiKey, config);
          break;
        case AIServiceProvider.Cerebras:
          service = new CerebrasService(effectiveApiKey, config);
          break;
        case AIServiceProvider.Ollama:
          service = new OllamaService(effectiveApiKey, config);
          break;
        case AIServiceProvider.OpenRouter:
          service = new OpenRouterService(effectiveApiKey, config);
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
      apiKey: effectiveApiKey,
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

  static invalidateService(
    provider: AIServiceProvider,
    apiKey: string,
    config?: AIServiceConfig,
  ): void {
    const instanceKey = this.buildInstanceKey(provider, apiKey, config);
    const existing = this.instances.get(instanceKey);
    if (!existing) {
      return;
    }

    try {
      existing.service.dispose();
    } finally {
      this.instances.delete(instanceKey);
    }
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

// Register factory for capability delegation
registerAIServiceFactory(AIServiceFactory);
