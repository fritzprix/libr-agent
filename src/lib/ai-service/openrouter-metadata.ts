/**
 * OpenRouter Metadata Service
 *
 * Uses OpenRouter's public /api/v1/models endpoint as a unified metadata database
 * for model capabilities across ALL providers (OpenAI, Anthropic, Google, etc.).
 *
 * CRITICAL: This does NOT route API calls through OpenRouter!
 * - Metadata queries: OpenRouter API (free, public)
 * - Actual chat calls: Direct to user's configured provider (Ollama, OpenAI, etc.)
 *
 * Benefits:
 * - Zero maintenance: OpenRouter team keeps model metadata up-to-date
 * - Unified schema: Single API for 400+ models across providers
 * - Real pricing data: pricing.internal_reasoning for reasoning token costs
 * - Capability detection: supported_parameters includes "reasoning", "tools", etc.
 */

import { getLogger } from '../logger';

const logger = getLogger('OpenRouterMetadata');

const OPENROUTER_MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';
const CACHE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

interface OpenRouterModel {
  id: string; // e.g., "openai/gpt-4o", "anthropic/claude-3.5-sonnet"
  name: string;
  description: string;
  context_length: number;
  architecture: {
    input_modalities: string[]; // ["text", "image"]
    output_modalities: string[]; // ["text"]
  };
  pricing: {
    prompt: string; // Cost per million prompt tokens
    completion: string; // Cost per million completion tokens
    internal_reasoning?: string; // Cost for reasoning tokens (e.g., o1/o3)
  };
  supported_parameters: string[]; // ["reasoning", "tools", "response_format", ...]
  top_provider: {
    max_completion_tokens?: number;
  };
}

interface OpenRouterResponse {
  data: OpenRouterModel[];
}

interface ModelMetadataCache {
  models: Map<string, OpenRouterModel>;
  fetchedAt: number;
}

let metadataCache: ModelMetadataCache | null = null;

/**
 * Fetch all model metadata from OpenRouter.
 * Caches results for 24 hours to minimize API calls.
 *
 * @returns Map of model IDs to model metadata
 */
async function fetchOpenRouterModels(): Promise<Map<string, OpenRouterModel>> {
  // Check cache validity
  if (metadataCache && Date.now() - metadataCache.fetchedAt < CACHE_TTL_MS) {
    logger.debug('Using cached OpenRouter metadata', {
      modelCount: metadataCache.models.size,
      cacheAge: Date.now() - metadataCache.fetchedAt,
    });
    return metadataCache.models;
  }

  try {
    logger.info('Fetching model metadata from OpenRouter API');
    const response = await fetch(OPENROUTER_MODELS_ENDPOINT, {
      method: 'GET',
      headers: {
        'Content-Type': 'application/json',
        // No API key needed for public models endpoint
      },
    });

    if (!response.ok) {
      throw new Error(
        `OpenRouter API error: ${response.status} ${response.statusText}`,
      );
    }

    const data: OpenRouterResponse = await response.json();
    const modelsMap = new Map<string, OpenRouterModel>();

    for (const model of data.data) {
      modelsMap.set(model.id, model);
    }

    // Update cache
    metadataCache = {
      models: modelsMap,
      fetchedAt: Date.now(),
    };

    logger.info('OpenRouter metadata cached', {
      modelCount: modelsMap.size,
    });

    return modelsMap;
  } catch (error) {
    logger.error('Failed to fetch OpenRouter metadata', error);

    // Return stale cache if available
    if (metadataCache) {
      logger.warn('Using stale OpenRouter cache due to fetch error');
      return metadataCache.models;
    }

    return new Map();
  }
}

/**
 * Find model metadata by model ID or name.
 * Handles multiple naming conventions:
 * - OpenRouter format: "openai/gpt-4o", "anthropic/claude-3.5-sonnet"
 * - Direct format: "gpt-4o", "claude-3.5-sonnet"
 * - Ollama format: "llama3.2", "qwen2.5-coder"
 *
 * @param modelId - Model identifier (any format)
 * @param provider - Provider hint to help match ambiguous names
 * @returns Model metadata if found
 */
export async function findModelMetadata(
  modelId: string,
  provider?: string,
): Promise<OpenRouterModel | null> {
  const models = await fetchOpenRouterModels();

  // Try exact match first (e.g., "openai/gpt-4o")
  if (models.has(modelId)) {
    return models.get(modelId)!;
  }

  // Try adding provider prefix
  if (provider && provider !== 'ollama' && provider !== 'openrouter') {
    const prefixedId = `${provider}/${modelId}`;
    if (models.has(prefixedId)) {
      return models.get(prefixedId)!;
    }
  }

  // Fuzzy search by name (case-insensitive)
  const normalizedQuery = modelId.toLowerCase();
  for (const [id, model] of models.entries()) {
    if (
      id.toLowerCase().includes(normalizedQuery) ||
      model.name.toLowerCase().includes(normalizedQuery)
    ) {
      return model;
    }
  }

  return null;
}

/**
 * Check if a model supports reasoning/thinking mode based on OpenRouter metadata.
 *
 * @param modelId - Model identifier
 * @param provider - Provider hint
 * @returns True if model supports reasoning
 */
export async function supportsReasoningViaOpenRouter(
  modelId: string,
  provider?: string,
): Promise<boolean> {
  const metadata = await findModelMetadata(modelId, provider);

  if (!metadata) {
    return false;
  }

  // Check if "reasoning" is in supported_parameters
  const hasReasoningParam = metadata.supported_parameters.includes('reasoning');

  // Check if internal_reasoning pricing exists (indicates reasoning capability)
  const hasReasoningPricing = !!metadata.pricing.internal_reasoning;

  return hasReasoningParam || hasReasoningPricing;
}

/**
 * Get context window size from OpenRouter metadata.
 *
 * @param modelId - Model identifier
 * @param provider - Provider hint
 * @returns Context window size in tokens, or null if not available
 */
export async function getContextLengthViaOpenRouter(
  modelId: string,
  provider?: string,
): Promise<number | null> {
  const metadata = await findModelMetadata(modelId, provider);

  if (!metadata?.context_length) {
    return null;
  }

  return metadata.context_length;
}

/**
 * Get reasoning token cost multiplier if available.
 *
 * @param modelId - Model identifier
 * @param provider - Provider hint
 * @returns Cost per million reasoning tokens, or null if not available
 */
export async function getReasoningTokenCost(
  modelId: string,
  provider?: string,
): Promise<number | null> {
  const metadata = await findModelMetadata(modelId, provider);

  if (!metadata?.pricing.internal_reasoning) {
    return null;
  }

  return parseFloat(metadata.pricing.internal_reasoning);
}

/**
 * Clear the metadata cache.
 * Useful for testing or forcing a refresh.
 */
export function clearMetadataCache(): void {
  metadataCache = null;
  logger.info('OpenRouter metadata cache cleared');
}
