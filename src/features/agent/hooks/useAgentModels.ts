import { useCallback, useMemo } from 'react';
import useSWR from 'swr';
import { toast } from 'sonner';

import { useSettings } from '@/hooks/use-settings';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import type { AIModelLookupService } from '@/lib/ai-service/types';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import { withTimeout } from '@/lib/retry-utils';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAgentModels');

export const useAgentModels = (provider?: string) => {
  const {
    value: { serviceConfigs },
  } = useSettings();

  // Get API key and baseUrl for the selected provider
  const apiKey = useMemo(() => {
    return serviceConfigs[provider as AIServiceProvider]?.apiKey || '';
  }, [serviceConfigs, provider]);

  const baseUrl = useMemo(() => {
    return serviceConfigs[provider as AIServiceProvider]?.baseUrl || '';
  }, [serviceConfigs, provider]);

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
        const providerConfig = serviceConfigs[p as AIServiceProvider] || {};
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
    [serviceConfigs],
  );

  const {
    data: dynamicModels = {},
    mutate: refreshModels,
    isValidating: isRefreshing,
  } = useSWR(
    provider ? ['local-models', provider, apiKey, baseUrl] : null,
    fetchDynamicModels,
    {
      revalidateOnFocus: false,
      dedupingInterval: 30000,
    },
  );

  // Combine static and dynamic models
  const availableModels = useMemo(() => {
    if (!provider) return {};

    const providerConfig = serviceConfigs[provider as AIServiceProvider] || {};

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
  }, [provider, dynamicModels, serviceConfigs]);

  return {
    availableModels,
    isRefreshing,
    refreshModels,
  };
};
