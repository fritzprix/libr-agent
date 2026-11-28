/**
 * Shared dynamic model listing for pickers / ModelProvider.
 * Keeps timeout, toast policy, and SWR credential fingerprinting in one place.
 */

import {
  isCustomOpenAIProviderId,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service/custom-providers';
import { listLlmModels, type ListedModel } from '@/lib/backend/llm-models';
import { stableHashKeyPart } from '@/lib/ai-service/base-service-utils';
import { reportListModelsFallback } from '@/lib/ai-service/list-models-errors';
import { getDynamicModelFetchPolicy } from '@/lib/ai-service/model-fetch-policy';
import { setStoredModelCache } from '@/lib/ai-service/model-cache-storage';
import { getContextWindow } from '@/lib/ai-service/model-capabilities';
import { getModelToolSupport } from '@/lib/ai-service/ollama-model-utils';
import {
  resolveProviderModels,
  type ProviderModelMap,
} from '@/lib/ai-service/resolve-provider-models';
import { AIServiceProvider } from '@/lib/ai-service/types';
import { llmConfigManager, type ModelInfo } from '@/lib/llm-config-manager';
import { getLogger } from '@/lib/logger';
import { withTimeout } from '@/lib/retry-utils';
import type { Settings } from '@/lib/services/settings-service';

const logger = getLogger('fetchDynamicProviderModels');

/** Background discovery: fail fast so one offline provider cannot stall the UI. */
export const BACKGROUND_LIST_MODELS_TIMEOUT_MS = 8_000;
/** Explicit Refresh: allow slower endpoints. */
export const REFRESH_LIST_MODELS_TIMEOUT_MS = 20_000;

export type ProviderModelsSettingsSlice = Pick<
  Settings,
  'serviceConfigs' | 'customProviders'
>;

export interface FetchDynamicProviderModelsOptions {
  timeoutMs: number;
  notifyUser: boolean;
}

/**
 * Fingerprint an API key for SWR cache keys without storing the raw secret.
 */
export function fingerprintCredentialForSwrKey(apiKey: string): string {
  if (!apiKey) {
    return '';
  }
  return stableHashKeyPart(apiKey);
}

/**
 * Stable per-provider segment for model-list SWR keys.
 * Format: `providerId|apiKeyFingerprint|baseUrl|party|customModelId`
 */
export function buildProviderModelsSwrSegment(args: {
  providerId: string;
  apiKey: string;
  baseUrl?: string | null;
  use3rdParty?: boolean;
  customModelId?: string | null;
}): string {
  return [
    args.providerId,
    fingerprintCredentialForSwrKey(args.apiKey),
    args.baseUrl ?? '',
    args.use3rdParty ? 'use-3rd-party' : 'first-party',
    args.customModelId ?? '',
  ].join('|');
}

/** The GUI's Ollama default host, used for per-model detail lookups. */
const OLLAMA_DEFAULT_API_BASE = 'http://localhost:11434';

/**
 * Providers that have a model list API in `llm-client` (#3091).
 * Providers without one (Groq, Cerebras) keep the static
 * catalog from the LLM config; their absence is not a failure.
 */
function providerHasModelListApi(providerId: string): boolean {
  return (
    providerId === AIServiceProvider.OpenAI ||
    providerId === AIServiceProvider.Anthropic ||
    providerId === AIServiceProvider.Gemini ||
    providerId === AIServiceProvider.Ollama ||
    providerId === AIServiceProvider.OpenRouter ||
    providerId === AIServiceProvider.Fireworks ||
    isCustomOpenAIProviderId(providerId)
  );
}

/**
 * Fills a [`ListedModel`] from the native list call into the GUI's
 * `ModelInfo` shape, merging the static LLM config and the dynamic
 * capability detection exactly as the provider services did before #3091.
 */
async function modelInfoFromListedModel(
  providerId: string,
  entry: ListedModel,
  ollamaApiBase: string,
): Promise<ModelInfo> {
  const id = entry.id;

  if (providerId === AIServiceProvider.Ollama) {
    return {
      id,
      name: id,
      contextWindow: await getContextWindow(id, AIServiceProvider.Ollama, {
        apiBase: ollamaApiBase,
      }),
      supportReasoning: true,
      supportTools: getModelToolSupport(id),
      supportStreaming: true,
      cost: { input: 0, output: 0 },
      description: entry.description || id || 'Ollama model',
    };
  }

  if (providerId === AIServiceProvider.OpenRouter) {
    // OpenRouter reports capabilities and pricing itself; no static merge.
    return {
      id,
      name: entry.name || id,
      contextWindow: entry.contextWindow ?? 4096,
      supportReasoning: entry.supportReasoning ?? false,
      supportTools: entry.supportTools ?? true,
      supportStreaming: entry.supportStreaming ?? true,
      cost: entry.cost ?? { input: 0, output: 0 },
      description: entry.description || entry.name || id,
    };
  }

  if (providerId === AIServiceProvider.Anthropic) {
    const staticModel = llmConfigManager.getModel('anthropic', id);
    return {
      id,
      name: entry.name || staticModel?.name || id,
      contextWindow: await getContextWindow(id, AIServiceProvider.Anthropic),
      supportReasoning: staticModel?.supportReasoning ?? false,
      supportTools: staticModel?.supportTools ?? true,
      supportStreaming: staticModel?.supportStreaming ?? true,
      cost: staticModel?.cost || { input: 0, output: 0 },
      description: staticModel?.description || `Claude model: ${id}`,
    };
  }

  if (providerId === AIServiceProvider.Gemini) {
    const staticModel = llmConfigManager.getModel('gemini', id);
    return {
      id,
      name: entry.name || staticModel?.name || id,
      contextWindow:
        entry.contextWindow || staticModel?.contextWindow || 1048576,
      supportReasoning:
        staticModel?.supportReasoning ??
        /gemini-2\.[5-9]|gemini-[3-9]/.test(id),
      supportTools: staticModel?.supportTools ?? true,
      supportStreaming: staticModel?.supportStreaming ?? true,
      cost: staticModel?.cost || { input: 0, output: 0 },
      description:
        entry.description || staticModel?.description || `Gemini model: ${id}`,
    };
  }

  // OpenAI and custom OpenAI-compatible providers: the static catalog is
  // keyed under `openai`, as the service mapped them. Fireworks inherits
  // `OpenAIService`, so it shares this mapping but keeps its own context
  // rules for the capability fallback.
  const staticModel = llmConfigManager.getModel('openai', id);
  const capabilityProvider =
    providerId === AIServiceProvider.Fireworks
      ? AIServiceProvider.Fireworks
      : AIServiceProvider.OpenAI;
  return {
    id,
    name: staticModel?.name || id,
    contextWindow: await getContextWindow(id, capabilityProvider),
    supportReasoning:
      staticModel?.supportReasoning ??
      (id.toLowerCase().includes('gpt-4') ||
        id.toLowerCase().includes('gpt-3.5')),
    supportTools: staticModel?.supportTools ?? false,
    supportStreaming: staticModel?.supportStreaming ?? true,
    cost: staticModel?.cost || { input: 0, output: 0 },
    description: staticModel?.description || entry.description || id,
  };
}

/**
 * Fetch `/models` (or provider equivalent) for one provider through the
 * native `list_llm_models` command.
 * On failure: silent or toast via `notifyUser`, then return `{}` so callers
 * can fall back through resolveProviderModels / local cache.
 */
export async function fetchDynamicProviderModels(
  providerId: string,
  settings: ProviderModelsSettingsSlice,
  options: FetchDynamicProviderModelsOptions,
): Promise<ProviderModelMap> {
  const resolved = resolveProviderRuntimeConfig(providerId, settings);
  const policy = getDynamicModelFetchPolicy({
    provider: providerId,
    apiKey: resolved.apiKey,
    baseUrl: resolved.baseUrl,
    use3rdParty: resolved.use3rdParty,
    customModelId: resolved.customModelId,
  });

  if (!policy.canFetch) {
    return {};
  }

  if (!providerHasModelListApi(providerId)) {
    return {};
  }

  try {
    // The whole discovery — native list call plus the per-model capability
    // lookups — must finish inside `options.timeoutMs`, and the lookups run
    // in parallel like the services' `listModels` did before #3091.
    const modelsRecord = await withTimeout(
      (async () => {
        const listed = await listLlmModels({
          providerId,
          timeoutMs: options.timeoutMs,
        });
        const entries = listed.filter((entry) => Boolean(entry.id));
        const pairs = await Promise.all(
          entries.map(async (entry) => {
            const info = await modelInfoFromListedModel(
              providerId,
              entry,
              resolved.baseUrl || OLLAMA_DEFAULT_API_BASE,
            );
            return [entry.id, info] as const;
          }),
        );
        const record: ProviderModelMap = {};
        for (const [id, info] of pairs) {
          record[id] = info;
        }
        return record;
      })(),
      options.timeoutMs,
    );

    // Gemini keeps the static-catalog models the API does not list, as the
    // service did.
    if (providerId === AIServiceProvider.Gemini) {
      const staticModels = llmConfigManager.getModelsForProvider(providerId);
      for (const [modelId, modelInfo] of Object.entries(staticModels ?? {})) {
        if (!(modelId in modelsRecord)) {
          modelsRecord[modelId] = modelInfo;
        }
      }
    }

    if (Object.keys(modelsRecord).length > 0) {
      setStoredModelCache(providerId, modelsRecord);
    }

    logger.debug(
      `Fetched ${Object.keys(modelsRecord).length} models from ${providerId}`,
    );
    return modelsRecord;
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    logger.warn(`Failed to fetch models for ${providerId}:`, errorMessage);
    const fallbackModels = resolveProviderModels(providerId, settings);
    reportListModelsFallback({
      provider: providerId,
      baseUrl: resolved.baseUrl,
      reason: 'api_error',
      error,
      hasCachedModels: Object.keys(fallbackModels).length > 0,
      notifyUser: options.notifyUser,
    });
    return {};
  }
}
