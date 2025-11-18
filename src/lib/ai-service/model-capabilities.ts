/**
 * Model capability detection utilities.
 * Provides dynamic detection for model features like thinking/reasoning support.
 *
 * Strategy:
 * 1. Primary: OpenRouter metadata API (unified source for all providers)
 * 2. Secondary: Provider-specific API (Ollama /api/show, etc.)
 * 3. Fallback: Minimal pattern matching for known model families
 * 4. Cache: Store results for 24 hours to avoid repeated API calls
 *
 * CRITICAL: Uses OpenRouter as "metadata database" WITHOUT routing API calls!
 * - Metadata: OpenRouter API (free, public endpoint)
 * - Actual chat: Direct to user's configured provider (Ollama, OpenAI, etc.)
 */

import { AIServiceProvider } from './types';
import { getLogger } from '../logger';
import {
  supportsReasoningViaOpenRouter,
  getContextLengthViaOpenRouter,
} from './openrouter-metadata';

const logger = getLogger('ModelCapabilities');

/**
 * Cache for model capabilities to avoid repeated API calls.
 * TTL: 24 hours
 */
interface CapabilityCache {
  thinking: boolean;
  tools: boolean;
  contextWindow: number;
  timestamp: number;
}

const CACHE_TTL = 24 * 60 * 60 * 1000; // 24 hours
const capabilityCache = new Map<string, CapabilityCache>();

/**
 * Minimal fallback patterns for known model families that support thinking.
 * Only includes most common/stable patterns to avoid maintenance burden.
 *
 * NOTE: This is a LAST RESORT fallback. Primary detection is via:
 * 1. OpenRouter metadata API
 * 2. Provider-specific APIs (Ollama /api/show)
 *
 * Keep this list MINIMAL - only add patterns for well-established model families.
 */
const FALLBACK_THINKING_PATTERNS: Record<string, string[]> = {
  [AIServiceProvider.Ollama]: [
    'qwen', // Qwen family (popular thinking models)
    'deepseek', // DeepSeek-R1 family
  ],
  [AIServiceProvider.OpenAI]: [
    'o1', // o1 reasoning series
    'o3', // o3 reasoning series
    'o4', // o4 reasoning series
  ],
  [AIServiceProvider.Anthropic]: [
    'claude-opus-4', // Claude 4 Opus with extended thinking
    'claude-sonnet-4', // Claude 4 Sonnet with extended thinking
  ],
  [AIServiceProvider.Gemini]: [
    'gemini-2.5', // Gemini 2.5 with thinking budget
    'gemini-2.0', // Gemini 2.0 experimental thinking
  ],
  // Most providers should be detected via OpenRouter API
  [AIServiceProvider.Groq]: [],
  [AIServiceProvider.Cerebras]: [],
  [AIServiceProvider.Fireworks]: [],
  [AIServiceProvider.Empty]: [],
};

/**
 * Checks if a cached capability is still valid.
 */
function isCacheValid(cached: CapabilityCache): boolean {
  return Date.now() - cached.timestamp < CACHE_TTL;
}

/**
 * Gets a cache key for a model.
 */
function getCacheKey(provider: AIServiceProvider, modelName: string): string {
  return `${provider}:${modelName}`;
}

/**
 * Fetches thinking capability from Ollama's /api/show endpoint.
 * This is the most reliable method for Ollama models.
 *
 * @param modelName - The Ollama model name
 * @param apiBase - Ollama server URL (default: http://localhost:11434)
 * @returns True if model supports thinking, false otherwise
 */
export async function fetchOllamaModelInfo(
  modelName: string,
  apiBase: string = 'http://localhost:11434',
): Promise<{ thinking: boolean; contextWindow?: number } | null> {
  try {
    const response = await fetch(`${apiBase}/api/show`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name: modelName }),
    });

    if (!response.ok) {
      logger.warn(`Failed to fetch Ollama model info for ${modelName}`);
      return null;
    }

    const data = await response.json();

    // Check if model has thinking parameter in modelfile or template
    const hasThinkingParam =
      data.modelfile?.toLowerCase().includes('think') ||
      data.template?.toLowerCase().includes('thinking') ||
      data.parameters?.think !== undefined;

    // Extract context window from parameters or model_info
    // Search for any *.context_length key in model_info (supports all model families)
    let contextWindow = data.parameters?.num_ctx;

    if (!contextWindow && data.model_info) {
      const contextEntry = Object.entries(data.model_info).find(([key]) =>
        key.endsWith('.context_length'),
      );
      contextWindow = contextEntry?.[1] as number | undefined;
    }

    logger.debug(`Ollama model info for ${modelName}`, {
      hasThinkingParam,
      contextWindow,
      modelfile: data.modelfile?.substring(0, 200), // Log first 200 chars
    });

    return {
      thinking: hasThinkingParam,
      contextWindow,
    };
  } catch (error) {
    logger.error(`Error fetching Ollama model info for ${modelName}:`, error);
    return null;
  }
}

/**
 * Detects if a model supports thinking/reasoning mode.
 * Uses a multi-tier approach:
 * 1. Check cache first
 * 2. Try OpenRouter metadata API (unified for all providers)
 * 3. For Ollama: Query /api/show endpoint
 * 4. Fallback: Minimal pattern matching
 *
 * @param modelName - The name of the model to check
 * @param provider - The AI service provider
 * @param options - Optional configuration (apiBase for Ollama)
 * @returns Promise<boolean> - True if the model likely supports thinking mode
 *
 * @example
 * ```typescript
 * const canThink = await supportsThinking('qwen2.5:latest', 'ollama');
 * const canThinkOpenAI = await supportsThinking('o3-mini', 'openai');
 * const canThinkViaOR = await supportsThinking('gpt-4o', 'openai'); // Uses OpenRouter metadata
 * ```
 */
export async function supportsThinking(
  modelName: string,
  provider: AIServiceProvider,
  options?: { apiBase?: string; skipCache?: boolean },
): Promise<boolean> {
  if (!modelName) {
    logger.warn('Empty model name provided for thinking support check');
    return false;
  }

  // Check cache first (unless explicitly skipped)
  if (!options?.skipCache) {
    const cacheKey = getCacheKey(provider, modelName);
    const cached = capabilityCache.get(cacheKey);
    if (cached && isCacheValid(cached)) {
      logger.debug(
        `Using cached thinking capability for ${modelName}:`,
        cached.thinking,
      );
      return cached.thinking;
    }
  }

  let thinking = false;

  // TIER 1: OpenRouter metadata API (works for ALL providers except Ollama)
  if (provider !== AIServiceProvider.Ollama) {
    try {
      thinking = await supportsReasoningViaOpenRouter(modelName, provider);
      if (thinking) {
        logger.info(
          `Detected thinking via OpenRouter for ${provider}/${modelName}`,
        );

        // Cache the result
        capabilityCache.set(getCacheKey(provider, modelName), {
          thinking: true,
          tools: true,
          contextWindow: 8192, // Will be overridden by actual API call
          timestamp: Date.now(),
        });

        return true;
      }
    } catch (error) {
      logger.warn(
        `OpenRouter metadata fetch failed for ${modelName}, falling back`,
        error,
      );
    }
  }

  // TIER 2: Ollama-specific API
  if (provider === AIServiceProvider.Ollama) {
    const modelInfo = await fetchOllamaModelInfo(modelName, options?.apiBase);
    if (modelInfo !== null) {
      thinking = modelInfo.thinking;

      // Cache the result
      capabilityCache.set(getCacheKey(provider, modelName), {
        thinking: modelInfo.thinking,
        tools: true,
        contextWindow: modelInfo.contextWindow || 4096,
        timestamp: Date.now(),
      });

      logger.info(
        `Detected thinking capability for ${modelName} via Ollama API:`,
        thinking,
      );
      return thinking;
    }
  }

  // TIER 3: Minimal pattern matching fallback
  const patterns = FALLBACK_THINKING_PATTERNS[provider] || [];
  const lowerName = modelName.toLowerCase();
  thinking = patterns.some((pattern) => lowerName.includes(pattern));

  // Cache fallback result
  capabilityCache.set(getCacheKey(provider, modelName), {
    thinking,
    tools: true,
    contextWindow: 4096,
    timestamp: Date.now(),
  });

  logger.debug(`Using fallback thinking detection for ${modelName}:`, thinking);
  return thinking;
}

/**
 * Gets the context window size for a model using dynamic detection.
 * Uses a multi-tier approach similar to supportsThinking():
 * 1. Check cache first
 * 2. Try OpenRouter metadata API (unified for all providers except Ollama)
 * 3. For Ollama: Query /api/show endpoint
 * 4. Fallback: estimateContextWindow() heuristic
 *
 * @param modelName - The name of the model to check
 * @param provider - The AI service provider
 * @param options - Optional configuration (apiBase for Ollama)
 * @returns Promise<number> - Context window size in tokens
 *
 * @example
 * ```typescript
 * const contextWindow = await getContextWindow('gpt-4o', 'openai');
 * const ollamaContext = await getContextWindow('llama3.1', 'ollama', { apiBase: 'http://localhost:11434' });
 * ```
 */
export async function getContextWindow(
  modelName: string,
  provider: AIServiceProvider,
  options?: { apiBase?: string; skipCache?: boolean },
): Promise<number> {
  if (!modelName) {
    logger.warn('Empty model name provided for context window check');
    return 32768; // Safe default
  }

  // Check cache first (unless explicitly skipped)
  if (!options?.skipCache) {
    const cacheKey = getCacheKey(provider, modelName);
    const cached = capabilityCache.get(cacheKey);
    if (cached && isCacheValid(cached)) {
      logger.debug(
        `Using cached context window for ${modelName}:`,
        cached.contextWindow,
      );
      return cached.contextWindow;
    }
  }

  let contextWindow = 32768; // Default fallback (safe for most modern models)

  // TIER 1: OpenRouter metadata API (works for ALL providers except Ollama)
  if (provider !== AIServiceProvider.Ollama) {
    try {
      const contextLength = await getContextLengthViaOpenRouter(
        modelName,
        provider,
      );
      if (contextLength !== null) {
        contextWindow = contextLength;
        logger.info(
          `Detected context window via OpenRouter for ${provider}/${modelName}: ${contextWindow}`,
        );

        // Update cache
        const cacheKey = getCacheKey(provider, modelName);
        const existing = capabilityCache.get(cacheKey);
        capabilityCache.set(cacheKey, {
          thinking: existing?.thinking ?? false,
          tools: existing?.tools ?? true,
          contextWindow,
          timestamp: Date.now(),
        });

        return contextWindow;
      }
    } catch (error) {
      logger.warn(
        `OpenRouter metadata fetch failed for ${modelName}, falling back`,
        error,
      );
    }
  }

  // TIER 2: Ollama-specific API
  if (provider === AIServiceProvider.Ollama) {
    const modelInfo = await fetchOllamaModelInfo(modelName, options?.apiBase);
    if (modelInfo !== null && modelInfo.contextWindow) {
      contextWindow = modelInfo.contextWindow;

      // Update cache
      const cacheKey = getCacheKey(provider, modelName);
      const existing = capabilityCache.get(cacheKey);
      capabilityCache.set(cacheKey, {
        thinking: existing?.thinking ?? modelInfo.thinking,
        tools: existing?.tools ?? true,
        contextWindow,
        timestamp: Date.now(),
      });

      logger.info(
        `Detected context window for ${modelName} via Ollama API: ${contextWindow}`,
      );
      return contextWindow;
    }
  }

  // TIER 3: Fallback heuristic
  contextWindow = estimateContextWindow(modelName, provider);

  // Cache fallback result
  const cacheKey = getCacheKey(provider, modelName);
  const existing = capabilityCache.get(cacheKey);
  capabilityCache.set(cacheKey, {
    thinking: existing?.thinking ?? false,
    tools: existing?.tools ?? true,
    contextWindow,
    timestamp: Date.now(),
  });

  logger.debug(
    `Using fallback context window estimation for ${modelName}: ${contextWindow}`,
  );
  return contextWindow;
}

/**
 * Gets the recommended thinking level for a model.
 * Some models have different default levels for optimal performance.
 *
 * @param modelName - The name of the model
 * @param provider - The AI service provider
 * @returns Recommended thinking level ('low' | 'medium' | 'high')
 */
/**
 * Get recommended reasoning level based on model capabilities.
 * Higher-tier models default to higher reasoning levels.
 *
 * @param modelName - The model identifier
 * @returns Recommended reasoning effort level
 */
export function getRecommendedReasoningLevel(
  modelName: string,
  provider: AIServiceProvider,
): 'low' | 'medium' | 'high' {
  const lowerName = modelName.toLowerCase();

  // OpenAI o3 models perform better with higher reasoning effort
  if (provider === AIServiceProvider.OpenAI && lowerName.includes('o3')) {
    return 'medium';
  }

  // DeepSeek R1 models benefit from higher thinking levels
  if (
    provider === AIServiceProvider.Ollama &&
    lowerName.includes('deepseek-r1')
  ) {
    return 'medium';
  }

  // Default to 'low' for cost/performance balance
  return 'low';
}

/**
 * Clears the capability cache for a specific model or all models.
 *
 * @param provider - Optional provider to clear cache for
 * @param modelName - Optional specific model to clear
 */
export function clearCapabilityCache(
  provider?: AIServiceProvider,
  modelName?: string,
): void {
  if (provider && modelName) {
    capabilityCache.delete(getCacheKey(provider, modelName));
  } else if (provider) {
    // Clear all models for this provider
    for (const key of capabilityCache.keys()) {
      if (key.startsWith(`${provider}:`)) {
        capabilityCache.delete(key);
      }
    }
  } else {
    // Clear all
    capabilityCache.clear();
  }
  logger.info('Cleared capability cache', { provider, modelName });
}

/**
 * Checks if a model supports tool use based on its name.
 * This is a heuristic approach similar to thinking support detection.
 *
 * @param modelName - The name of the model to check
 * @param provider - The AI service provider
 * @returns True if the model likely supports tool calling
 */
export function supportsTools(
  modelName: string,
  provider: AIServiceProvider,
): boolean {
  const lowerName = modelName.toLowerCase();

  switch (provider) {
    case AIServiceProvider.Ollama: {
      // Most modern Ollama models support tools
      const noToolModels = ['llama2', 'codellama:7b'];
      return !noToolModels.some((m) => lowerName.includes(m));
    }

    case AIServiceProvider.OpenAI:
      // All GPT-4, GPT-3.5-turbo, and o-series support tools
      return (
        lowerName.includes('gpt-4') ||
        lowerName.includes('gpt-3.5-turbo') ||
        lowerName.startsWith('o')
      );

    case AIServiceProvider.Anthropic:
      // Claude 3+ supports tools
      return (
        lowerName.includes('claude-3') || lowerName.includes('claude-opus')
      );

    case AIServiceProvider.Gemini:
      // Gemini 1.5+ supports tools
      return lowerName.includes('gemini-1.5') || lowerName.includes('gemini-2');

    default:
      return false;
  }
}

/**
 * Estimates context window size based on model name.
 * This is a fallback when API doesn't provide this information.
 *
 * @param modelName - The name of the model
 * @param provider - The AI service provider
 * @returns Estimated context window size in tokens
 */
export function estimateContextWindow(
  modelName: string,
  provider: AIServiceProvider,
): number {
  const lowerName = modelName.toLowerCase();

  switch (provider) {
    case AIServiceProvider.Ollama:
      // Ollama models should be detected via /api/show endpoint
      // This fallback should rarely be used
      return 32768; // Safe default to avoid context overflow

    case AIServiceProvider.OpenAI:
      if (lowerName.includes('gpt-4.1')) return 1000000;
      if (lowerName.includes('gpt-4o')) return 128000;
      if (lowerName.includes('o3') || lowerName.includes('o4')) return 200000;
      return 8192;

    case AIServiceProvider.Anthropic:
      if (lowerName.includes('claude-3')) return 200000;
      return 100000;

    case AIServiceProvider.Gemini:
      if (lowerName.includes('gemini-1.5') || lowerName.includes('gemini-2'))
        return 1000000;
      return 32000;

    default:
      return 4096;
  }
}
