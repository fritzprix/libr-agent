/**
 * Static per-provider model capabilities (#3092).
 *
 * `supportsTools` / `estimateContextWindow` used to ask the per-provider AI
 * services through `AIServiceFactory.getCapabilityDelegate`. The services are
 * being deleted, so the same per-provider logic lives here as plain data +
 * predicates instead of delegating through service classes.
 *
 * Routing rules preserved from `AIServiceFactory.resolveFactoryProvider`:
 * `custom:*` ids use the OpenAI-compatible logic, and an unknown provider id
 * falls back to the `empty` behavior (no tools, zero context window) — the
 * same results the factory delegation produced.
 */
import { AIServiceProvider } from './types';
import { getModelToolSupport } from './ollama-model-utils';

const CUSTOM_PROVIDER_PREFIX = 'custom:';

function isCustomProviderId(provider: string): boolean {
  return (
    provider.startsWith(CUSTOM_PROVIDER_PREFIX) &&
    provider.length > CUSTOM_PROVIDER_PREFIX.length
  );
}

/** Tool-support predicates, keyed by the provider id used at the call site. */
const TOOL_SUPPORT = new Map<string, (modelName: string) => boolean>([
  [
    AIServiceProvider.Groq,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      return (
        lowerName.includes('llama3') ||
        lowerName.includes('llama-3') ||
        lowerName.includes('mixtral')
      );
    },
  ],
  [
    AIServiceProvider.OpenAI,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      const isReasoningModel = /^o(?:1|3|4)(?:$|[-.])/.test(lowerName);
      return (
        lowerName.includes('gpt-4') ||
        lowerName.includes('gpt-3.5-turbo') ||
        isReasoningModel
      );
    },
  ],
  [
    AIServiceProvider.Anthropic,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      return (
        lowerName.includes('claude-3') || lowerName.includes('claude-opus')
      );
    },
  ],
  [
    AIServiceProvider.Gemini,
    (modelName) => /gemini-(1\.[5-9]|[2-9])/.test(modelName.toLowerCase()),
  ],
  [AIServiceProvider.Fireworks, () => true],
  [
    AIServiceProvider.Cerebras,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      return lowerName.includes('llama3.1') || lowerName.includes('llama-3.1');
    },
  ],
  [AIServiceProvider.Ollama, (modelName) => getModelToolSupport(modelName)],
  [AIServiceProvider.OpenRouter, () => true],
  [AIServiceProvider.Empty, () => false],
]);

/** Context-window estimators, keyed by the provider id used at the call site. */
const CONTEXT_WINDOW = new Map<string, (modelName: string) => number>([
  [
    AIServiceProvider.Groq,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('llama-3.1-70b')) return 128000;
      if (lowerName.includes('llama-3.1-8b')) return 128000;
      return 32768;
    },
  ],
  [
    AIServiceProvider.OpenAI,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('gpt-4.1')) return 1000000;
      if (lowerName.includes('gpt-4o')) return 128000;
      if (/^o(?:3|4)(?:$|[-.])/.test(lowerName)) return 200000;
      return 8192;
    },
  ],
  [
    AIServiceProvider.Anthropic,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('claude-3')) return 200000;
      return 100000;
    },
  ],
  [
    AIServiceProvider.Gemini,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('gemini-1.5-pro')) return 2000000;
      if (lowerName.includes('gemini-1.5-flash')) return 1000000;
      if (lowerName.includes('gemini-2.0-flash')) return 1000000;
      return 1000000;
    },
  ],
  [
    AIServiceProvider.Fireworks,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('qwen')) return 32768;
      return 128000;
    },
  ],
  [
    AIServiceProvider.Cerebras,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('llama3.1-70b')) return 131072;
      if (lowerName.includes('llama3.1-8b')) return 131072;
      return 8192;
    },
  ],
  [
    AIServiceProvider.Ollama,
    (modelName) => {
      const lowerName = modelName.toLowerCase();
      if (lowerName.includes('llama-3.1-405b')) return 128000;
      if (lowerName.includes('llama-3.1')) return 128000;
      if (lowerName.includes('llama-3.2')) return 128000;
      if (lowerName.includes('mistral-nemo')) return 128000;
      if (lowerName.includes('command-r')) return 128000;
      return 32768;
    },
  ],
  [AIServiceProvider.OpenRouter, () => 128000],
  [AIServiceProvider.Empty, () => 0],
]);

/**
 * Maps a call-site provider id to the capability key: `custom:*` ids resolve
 * to the OpenAI-compatible logic, unknown ids to `empty`.
 */
function resolveCapabilityKey(provider: AIServiceProvider | string): string {
  if (isCustomProviderId(provider)) {
    return AIServiceProvider.OpenAI;
  }
  if (TOOL_SUPPORT.has(provider)) {
    return provider;
  }
  return AIServiceProvider.Empty;
}

/** Whether `modelName` supports tool use on `provider` (static data). */
export function providerSupportsTools(
  provider: AIServiceProvider | string,
  modelName: string,
): boolean {
  // `resolveCapabilityKey` only returns keys present in the map.
  return TOOL_SUPPORT.get(resolveCapabilityKey(provider))!(modelName);
}

/** Estimated context window for `modelName` on `provider` (static data). */
export function providerEstimateContextWindow(
  provider: AIServiceProvider | string,
  modelName: string,
): number {
  return CONTEXT_WINDOW.get(resolveCapabilityKey(provider))!(modelName);
}
