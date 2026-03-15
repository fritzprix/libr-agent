import { AIServiceProvider } from './types';
import { getLogger } from '../logger';
import {
  supportsReasoningViaOpenRouter,
  getContextLengthViaOpenRouter,
} from './openrouter-metadata';

const logger = getLogger('ModelCapabilities');

/**
 * Interface matching the static part of AIServiceFactory that we need.
 * This avoids a direct import and thus prevents circular dependencies.
 */
interface FactoryInterface {
  getCapabilityDelegate(provider: AIServiceProvider): ServiceInterface;
}

interface ServiceInterface {
  supportsTools(modelName: string): boolean;
  estimateContextWindow(modelName: string): number;
}

let registeredFactory: FactoryInterface | null = null;

/**
 * Registers the AIServiceFactory to be used for late-bound capability delegation.
 * This pattern avoids circular dependencies: Services -> Capabilities -> Factory -> Services.
 * @param factory The AIServiceFactory implementation.
 */
export function registerAIServiceFactory(factory: FactoryInterface): void {
  registeredFactory = factory;
}

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

function isCacheValid(cached: CapabilityCache): boolean {
  return Date.now() - cached.timestamp < CACHE_TTL;
}

function getCacheKey(provider: AIServiceProvider, modelName: string): string {
  return `${provider}:${modelName}`;
}

/**
 * Minimal fallback patterns for known model families that support thinking.
 */
const FALLBACK_THINKING_PATTERNS: Record<string, string[]> = {
  [AIServiceProvider.Ollama]: ['qwen', 'deepseek'],
  [AIServiceProvider.OpenAI]: ['o1', 'o3', 'o4'],
  [AIServiceProvider.Anthropic]: ['claude-opus-4', 'claude-sonnet-4'],
  [AIServiceProvider.Gemini]: ['gemini-2.5', 'gemini-2.0'],
  [AIServiceProvider.Groq]: [],
  [AIServiceProvider.Cerebras]: [],
  [AIServiceProvider.Fireworks]: [],
  [AIServiceProvider.Empty]: [],
};

/**
 * Fetches thinking capability from Ollama's /api/show endpoint.
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

    const hasThinkingParam =
      data.modelfile?.toLowerCase().includes('think') ||
      data.template?.toLowerCase().includes('thinking') ||
      data.parameters?.think !== undefined;

    let contextWindow = data.parameters?.num_ctx;

    if (!contextWindow && data.model_info) {
      const contextEntry = Object.entries(data.model_info).find(([key]) =>
        key.endsWith('.context_length'),
      );
      contextWindow = contextEntry?.[1] as number | undefined;
    }

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
 */
export async function supportsThinking(
  modelName: string,
  provider: AIServiceProvider,
  options?: { apiBase?: string; skipCache?: boolean },
): Promise<boolean> {
  if (!modelName) return false;

  if (!options?.skipCache) {
    const cacheKey = getCacheKey(provider, modelName);
    const cached = capabilityCache.get(cacheKey);
    if (cached && isCacheValid(cached)) return cached.thinking;
  }

  let thinking = false;

  if (provider !== AIServiceProvider.Ollama) {
    try {
      thinking = await supportsReasoningViaOpenRouter(modelName, provider);
      if (thinking) {
        capabilityCache.set(getCacheKey(provider, modelName), {
          thinking: true,
          tools: true,
          contextWindow: 8192,
          timestamp: Date.now(),
        });
        return true;
      }
    } catch (error) {
      logger.warn(`OpenRouter metadata fetch failed for ${modelName}`, error);
    }
  }

  if (provider === AIServiceProvider.Ollama) {
    const modelInfo = await fetchOllamaModelInfo(modelName, options?.apiBase);
    if (modelInfo !== null) {
      thinking = modelInfo.thinking;
      capabilityCache.set(getCacheKey(provider, modelName), {
        thinking: modelInfo.thinking,
        tools: true,
        contextWindow: modelInfo.contextWindow || 4096,
        timestamp: Date.now(),
      });
      return thinking;
    }
  }

  const patterns = FALLBACK_THINKING_PATTERNS[provider] || [];
  const lowerName = modelName.toLowerCase();
  thinking = patterns.some((pattern) => lowerName.includes(pattern));

  capabilityCache.set(getCacheKey(provider, modelName), {
    thinking,
    tools: true,
    contextWindow: 4096,
    timestamp: Date.now(),
  });

  return thinking;
}

/**
 * Gets the context window size for a model using dynamic detection.
 */
export async function getContextWindow(
  modelName: string,
  provider: AIServiceProvider,
  options?: { apiBase?: string; skipCache?: boolean },
): Promise<number> {
  if (!modelName) return 32768;

  if (!options?.skipCache) {
    const cacheKey = getCacheKey(provider, modelName);
    const cached = capabilityCache.get(cacheKey);
    if (cached && isCacheValid(cached)) return cached.contextWindow;
  }

  let contextWindow: number;

  if (provider !== AIServiceProvider.Ollama) {
    try {
      const contextLength = await getContextLengthViaOpenRouter(
        modelName,
        provider,
      );
      if (contextLength !== null) {
        contextWindow = contextLength;
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
      logger.warn(`OpenRouter metadata fetch failed for ${modelName}`, error);
    }
  }

  if (provider === AIServiceProvider.Ollama) {
    const modelInfo = await fetchOllamaModelInfo(modelName, options?.apiBase);
    if (modelInfo !== null && modelInfo.contextWindow) {
      contextWindow = modelInfo.contextWindow;
      const cacheKey = getCacheKey(provider, modelName);
      const existing = capabilityCache.get(cacheKey);
      capabilityCache.set(cacheKey, {
        thinking: existing?.thinking ?? modelInfo.thinking,
        tools: existing?.tools ?? true,
        contextWindow,
        timestamp: Date.now(),
      });
      return contextWindow;
    }
  }

  contextWindow = estimateContextWindow(modelName, provider);

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

/**
 * Checks if a model supports tool use based on its name.
 * This is now delegated to the respective service class via late-bound factory access.
 */
export function supportsTools(
  modelName: string,
  provider: AIServiceProvider,
): boolean {
  try {
    if (registeredFactory) {
      const service = registeredFactory.getCapabilityDelegate(provider);
      return service.supportsTools(modelName);
    }
    throw new Error('AIServiceFactory not registered');
  } catch {
    // Fallback if factory or service logic fails
    const lowerName = modelName.toLowerCase();
    if (provider === AIServiceProvider.OpenAI) {
      return (
        lowerName.includes('gpt-4') ||
        lowerName.includes('gpt-3.5-turbo') ||
        /^o(?:1|3|4)(?:$|[-.])/.test(lowerName)
      );
    }
    if (provider === AIServiceProvider.Anthropic)
      return lowerName.includes('claude-3');
    if (provider === AIServiceProvider.Gemini) return true;
    return false;
  }
}

/**
 * Estimates context window size based on model name.
 * This is now delegated to the respective service class via late-bound factory access.
 */
export function estimateContextWindow(
  modelName: string,
  provider: AIServiceProvider,
): number {
  try {
    if (registeredFactory) {
      const service = registeredFactory.getCapabilityDelegate(provider);
      return service.estimateContextWindow(modelName);
    }
    throw new Error('AIServiceFactory not registered');
  } catch {
    // Basic heuristics if delegation fails
    if (provider === AIServiceProvider.Anthropic) return 200000;
    if (provider === AIServiceProvider.OpenAI) return 128000;
    return 4096;
  }
}

/**
 * Clear the capability cache.
 */
export function clearCapabilityCache(
  provider?: AIServiceProvider,
  modelName?: string,
): void {
  if (provider && modelName) {
    capabilityCache.delete(getCacheKey(provider, modelName));
  } else if (provider) {
    for (const key of capabilityCache.keys()) {
      if (key.startsWith(`${provider}:`)) {
        capabilityCache.delete(key);
      }
    }
  } else {
    capabilityCache.clear();
  }
}

/**
 * Get recommended reasoning level based on model capabilities.
 */
export function getRecommendedReasoningLevel(
  modelName: string,
  provider: AIServiceProvider,
): 'low' | 'medium' | 'high' {
  const lowerName = modelName.toLowerCase();

  if (provider === AIServiceProvider.OpenAI && lowerName.includes('o3')) {
    return 'medium';
  }

  if (
    provider === AIServiceProvider.Ollama &&
    lowerName.includes('deepseek-r1')
  ) {
    return 'medium';
  }

  return 'low';
}
