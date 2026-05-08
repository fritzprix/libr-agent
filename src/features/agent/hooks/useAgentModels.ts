import { useCallback, useMemo } from 'react';
import useSWR from 'swr';
import { toast } from 'sonner';

import { useSettings } from '@/hooks/use-settings';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import { shouldFetchDynamicModels } from '@/lib/ai-service/model-fetch-policy';
import type { AIModelLookupService } from '@/lib/ai-service/types';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { withTimeout } from '@/lib/retry-utils';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAgentModels');

export const useAgentModels = (provider?: string) => {
  const {
    value: { serviceConfigs },
  } = useSettings();

  const providerConfig = useMemo(() => {
    if (!provider) {
      return {};
    }

    return {
      ...(serviceConfigs[provider as AIServiceProvider] || {}),
    };
  }, [provider, serviceConfigs]);

  // Get API key and baseUrl for the selected provider
  const apiKey = useMemo(() => {
    return providerConfig.apiKey || '';
  }, [providerConfig]);

  const baseUrl = useMemo(() => {
    return providerConfig.baseUrl || '';
  }, [providerConfig]);

  // Fetcher for models — delegates entirely to service.listModels().
  // Each provider's implementation decides static vs dynamic;
  // no hardcoded allowlist needed here.
  const fetchDynamicModels = useCallback(
    async ([, p, key]: [string, string, string]) => {
      // Use a non-empty placeholder so validateApiKey() doesn't throw.
      // Services that need a real key will fail gracefully in their API calls;
      // services using public endpoints (OpenRouter) or no key (Ollama) work normally.
      const effectiveApiKey = key || 'no-api-key';

      try {
        const service: AIModelLookupService = AIServiceFactory.getService(
          p as AIServiceProvider,
          effectiveApiKey,
          providerConfig,
        );
        const modelList = await withTimeout(service.listModels(), 20000);

        return modelList.reduce(
          (acc, modelInfo) => {
            const k = modelInfo.id || modelInfo.name;
            acc[k] = modelInfo;
            return acc;
          },
          {} as Record<string, ModelInfo>,
        );
      } catch (error) {
        logger.error('Failed to fetch models locally:', error);
        toast.error(`Failed to fetch models for ${p}`);
        return {};
      }
    },
    [providerConfig],
  );

  const {
    data: dynamicModels = {},
    mutate: refreshModels,
    isValidating: isRefreshing,
  } = useSWR(
    shouldFetchDynamicModels({
      provider,
      apiKey,
      use3rdParty: providerConfig.use3rdParty,
      customModelId: providerConfig.customModelId,
    })
      ? ['local-models', provider, apiKey, baseUrl]
      : null,
    fetchDynamicModels,
    {
      revalidateOnFocus: false,
      dedupingInterval: 30000,
    },
  );

  // Combine static and dynamic models
  const availableModels = useMemo(() => {
    if (!provider) return {};

    // If 3rd party is enabled for OpenAI, show only custom model ID
    if (
      provider === AIServiceProvider.OpenAI &&
      providerConfig.use3rdParty &&
      providerConfig.customModelId
    ) {
      const customModel: ModelInfo = {
        id: providerConfig.customModelId,
        name: providerConfig.customModelId,
        contextWindow: 128000,
        supportReasoning: false,
        supportTools: true,
        supportStreaming: true,
        cost: {
          input: 0,
          output: 0,
        },
        description: 'Custom 3rd party OpenAI-compatible model',
      };

      return {
        [providerConfig.customModelId]: customModel,
      };
    }

    // Otherwise, show static or dynamic models
    const staticModels =
      llmConfigManager.getModelsForProvider(provider as AIServiceProvider) ||
      {};

    return Object.keys(dynamicModels).length > 0 ? dynamicModels : staticModels;
  }, [provider, dynamicModels, providerConfig]);

  return {
    availableModels,
    isRefreshing,
    refreshModels,
  };
};
