import { AIServiceProvider } from './types';
import { isCustomOpenAIProviderId } from './custom-providers';
import { getModelToolSupport } from './ollama-model-utils';

/**
 * Plain static lookup tables for model capabilities (tool support and context
 * window estimation). This module has no dependencies on services or the
 * service factory, so any caller can use it without circular imports.
 */

/**
 * Maps a provider id (builtin enum value or `custom:<id>` endpoint id) to the
 * capability ruleset provider. Custom OpenAI-compatible endpoints use the
 * OpenAI rules; unknown ids fall back to `Empty` (no capabilities).
 */
export function resolveCapabilityProvider(
  provider: AIServiceProvider | string,
): AIServiceProvider {
  if (isCustomOpenAIProviderId(provider)) {
    return AIServiceProvider.OpenAI;
  }
  if (
    Object.values(AIServiceProvider).includes(provider as AIServiceProvider)
  ) {
    return provider as AIServiceProvider;
  }
  return AIServiceProvider.Empty;
}

/**
 * Checks whether a model supports tool/function calling for a provider.
 */
export function supportsToolsForModel(
  provider: AIServiceProvider | string,
  modelName: string,
): boolean {
  const lowerName = modelName.toLowerCase();

  switch (resolveCapabilityProvider(provider)) {
    case AIServiceProvider.Groq:
      return (
        lowerName.includes('llama3') ||
        lowerName.includes('llama-3') ||
        lowerName.includes('mixtral')
      );
    case AIServiceProvider.OpenAI:
      return (
        lowerName.includes('gpt-4') ||
        lowerName.includes('gpt-3.5-turbo') ||
        /^o(?:1|3|4)(?:$|[-.])/.test(lowerName)
      );
    case AIServiceProvider.Anthropic:
      return (
        lowerName.includes('claude-3') || lowerName.includes('claude-opus')
      );
    case AIServiceProvider.Gemini:
      return /gemini-(1\.[5-9]|[2-9])/.test(lowerName);
    case AIServiceProvider.Fireworks:
      return true;
    case AIServiceProvider.Cerebras:
      return lowerName.includes('llama3.1') || lowerName.includes('llama-3.1');
    case AIServiceProvider.Ollama:
      return getModelToolSupport(modelName);
    case AIServiceProvider.OpenRouter:
      return true;
    case AIServiceProvider.Empty:
    default:
      return false;
  }
}

/**
 * Estimates the context window size for a model on a provider.
 */
export function estimateContextWindowForModel(
  provider: AIServiceProvider | string,
  modelName: string,
): number {
  const lowerName = modelName.toLowerCase();

  switch (resolveCapabilityProvider(provider)) {
    case AIServiceProvider.Groq:
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('llama-3.1-70b')) return 128000;
      if (lowerName.includes('llama-3.1-8b')) return 128000;
      return 32768;
    case AIServiceProvider.OpenAI:
      if (lowerName.includes('gpt-4.1')) return 1000000;
      if (lowerName.includes('gpt-4o')) return 128000;
      if (/^o(?:3|4)(?:$|[-.])/.test(lowerName)) return 200000;
      return 8192;
    case AIServiceProvider.Anthropic:
      if (lowerName.includes('claude-3')) return 200000;
      return 100000;
    case AIServiceProvider.Gemini:
      if (lowerName.includes('gemini-1.5-pro')) return 2000000;
      return 1000000;
    case AIServiceProvider.Fireworks:
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('qwen')) return 32768;
      return 128000;
    case AIServiceProvider.Cerebras:
      if (lowerName.includes('llama3.1-70b')) return 131072;
      if (lowerName.includes('llama3.1-8b')) return 131072;
      return 8192;
    case AIServiceProvider.Ollama:
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('llama-3.1')) return 128000;
      if (lowerName.includes('llama-3.2')) return 128000;
      if (lowerName.includes('mistral-nemo')) return 128000;
      if (lowerName.includes('command-r')) return 128000;
      return 32768;
    case AIServiceProvider.OpenRouter:
      return 128000;
    case AIServiceProvider.Empty:
    default:
      return 0;
  }
}
