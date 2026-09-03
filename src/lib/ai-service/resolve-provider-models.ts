import {
  AIServiceProvider,
  isCustomOpenAIProviderId,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service';
import { getStoredModelCache } from '@/lib/ai-service/model-cache-storage';
import { llmConfigManager, type ModelInfo } from '@/lib/llm-config-manager';
import type { Settings } from '@/lib/services/settings-service';

export type ProviderModelMap = Record<string, ModelInfo>;

function manualModelsToMap(modelIds: string[] | undefined): ProviderModelMap {
  if (!modelIds || modelIds.length === 0) {
    return {};
  }

  return modelIds.reduce<ProviderModelMap>((acc, modelId) => {
    acc[modelId] = {
      id: modelId,
      name: modelId,
      contextWindow: 128000,
      supportReasoning: false,
      supportTools: true,
      supportStreaming: true,
      cost: { input: 0, output: 0 },
      description: 'Custom OpenAI-compatible model',
    };
    return acc;
  }, {});
}

/**
 * Resolves the model catalog for a single provider from static config, cache,
 * and optional dynamic fetch results.
 */
export function resolveProviderModels(
  providerId: string,
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
  dynamicModels: ProviderModelMap = {},
): ProviderModelMap {
  const resolved = resolveProviderRuntimeConfig(providerId, settings);
  if (!resolved) {
    return {};
  }

  if (
    providerId === AIServiceProvider.OpenAI &&
    resolved.use3rdParty &&
    resolved.customModelId
  ) {
    const customModel: ModelInfo = {
      id: resolved.customModelId,
      name: resolved.customModelId,
      contextWindow: 128000,
      supportReasoning: false,
      supportTools: true,
      supportStreaming: true,
      cost: { input: 0, output: 0 },
      description: 'Custom 3rd party OpenAI-compatible model',
    };

    return {
      [resolved.customModelId]: customModel,
    };
  }

  const manualModels = manualModelsToMap(resolved.manualModels);
  const staticModels = isCustomOpenAIProviderId(providerId)
    ? {}
    : llmConfigManager.getModelsForProvider(providerId as AIServiceProvider) ||
      {};
  const storedModels = getStoredModelCache(providerId);

  if (Object.keys(dynamicModels).length > 0) {
    return { ...manualModels, ...dynamicModels };
  }

  if (Object.keys(manualModels).length > 0) {
    return manualModels;
  }

  if (storedModels && Object.keys(storedModels).length > 0) {
    return storedModels;
  }

  return staticModels;
}
