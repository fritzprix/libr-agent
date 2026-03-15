import OpenAI from 'openai';
import { AIServiceProvider, AIServiceConfig } from './types';
import { OpenAIService } from './openai';
import { fetchOpenRouterModels } from './openrouter-metadata';
import type { ModelInfo } from '../llm-config-manager';
import { getLogger } from '../logger';
import { Message } from '@/models/chat';

const logger = getLogger('OpenRouterService');

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

  /**
   * @inheritdoc
   */
  sanitizeSingleMessage(message: Message): Message | null {
    // OpenRouter is a proxy, but we should strip thinking fields unless it's a known provider that supports it.
    // However, the proxy might handle it. For now, we strip to be safe as per base service pattern.
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
  supportsTools(modelName: string): boolean {
    // OpenRouter handles 200+ models. We return true here and rely on the UI/capabilities check if possible.
    // In practice, many models on OpenRouter support tools.
    void modelName;
    return true;
  }

  /**
   * @inheritdoc
   */
  estimateContextWindow(modelName: string): number {
    // listModels() provides the exact context window, but this sync method needs a heuristic or default
    void modelName;
    return 128000; // Common default for modern models on OpenRouter
  }

  /**
   * Fetches the full list of available models directly from OpenRouter's public
   * `/api/v1/models` endpoint (no API key required).
   *
   * Overrides the inherited `OpenAIService.listModels()` to avoid a redundant
   * double-fetch: the parent implementation calls `openai.models.list()` (auth
   * required) and then calls `getContextWindow()` which internally queries the
   * same OpenRouter public endpoint again.  Here we go straight to the source
   * and reuse the 24-hour metadata cache in `openrouter-metadata.ts`.
   *
   * @returns A promise resolving to an array of `ModelInfo` objects.
   */
  async listModels(): Promise<ModelInfo[]> {
    try {
      logger.info('Fetching models from OpenRouter public metadata API');
      const models = await fetchOpenRouterModels();

      const result: ModelInfo[] = Array.from(models.values()).map((m) => ({
        id: m.id,
        name: m.name,
        contextWindow: m.context_length ?? 4096,
        supportReasoning:
          m.supported_parameters.includes('reasoning') ||
          !!m.pricing.internal_reasoning,
        supportTools: m.supported_parameters.includes('tools'),
        supportStreaming:
          m.supported_parameters.includes('stream') ||
          m.supported_parameters.includes('streaming'),
        cost: {
          // OpenRouter pricing is per-token; convert to per-million for ModelInfo
          input: parseFloat(m.pricing.prompt ?? '0') * 1_000_000,
          output: parseFloat(m.pricing.completion ?? '0') * 1_000_000,
        },
        description: m.description || m.name,
      }));

      logger.info(`OpenRouter: ${result.length} models available`);
      return result;
    } catch (error) {
      logger.error(
        'Failed to fetch OpenRouter model list, falling back to static config',
        error,
      );
      return super.listModels();
    }
  }
}
