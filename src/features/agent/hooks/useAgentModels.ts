import { useCallback, useMemo } from 'react';
import useSWR from 'swr';

import { useSettings } from '@/hooks/use-settings';
import {
  AIServiceFactory,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service';
import { getDynamicModelFetchPolicy } from '@/lib/ai-service/model-fetch-policy';
import type { AIModelLookupService } from '@/lib/ai-service/types';
import { ModelInfo } from '@/lib/llm-config-manager';
import { withTimeout } from '@/lib/retry-utils';
import { getLogger } from '@/lib/logger';
import { reportListModelsFallback } from '@/lib/ai-service/list-models-errors';
import {
  getStoredModelCache,
  setStoredModelCache,
} from '@/lib/ai-service/model-cache-storage';
import {
  resolveProviderModels,
  type ProviderModelMap,
} from '@/lib/ai-service/resolve-provider-models';

const logger = getLogger('useAgentModels');
type DynamicModelMap = ProviderModelMap;
type ModelSWRKey = readonly [string, string, string, string, string, string];

/**
 * Loads models for a provider using **persisted** settings only.
 * Draft/unsaved API edits must not change the SWR key — refresh happens
 * after Settings save (via SettingsContext update + cache invalidation).
 */
export const useAgentModels = (provider?: string) => {
  const {
    value: { serviceConfigs, customProviders },
  } = useSettings();

  const resolved = useMemo(() => {
    if (!provider) {
      return null;
    }
    return resolveProviderRuntimeConfig(provider, {
      serviceConfigs,
      customProviders,
    });
  }, [provider, serviceConfigs, customProviders]);

  const apiKey = resolved?.apiKey ?? '';
  const baseUrl = resolved?.baseUrl ?? '';

  const dynamicModelPolicy = useMemo(
    () =>
      getDynamicModelFetchPolicy({
        provider,
        apiKey,
        baseUrl,
        use3rdParty: resolved?.use3rdParty,
        customModelId: resolved?.customModelId,
      }),
    [apiKey, baseUrl, provider, resolved?.customModelId, resolved?.use3rdParty],
  );

  const swrKey = useMemo<ModelSWRKey | null>(() => {
    if (!provider || !dynamicModelPolicy.canFetch || !resolved) {
      return null;
    }

    return [
      'local-models',
      provider,
      apiKey,
      baseUrl,
      resolved.use3rdParty ? 'use-3rd-party' : 'first-party',
      resolved.customModelId || '',
    ] as const;
  }, [apiKey, baseUrl, dynamicModelPolicy.canFetch, provider, resolved]);

  const fetchDynamicModels = useCallback(
    async ([, p, key]: ModelSWRKey) => {
      if (!resolved) {
        return {};
      }

      // Use a non-empty placeholder so validateApiKey() doesn't throw.
      const effectiveApiKey = key || 'no-api-key';

      try {
        const service: AIModelLookupService = AIServiceFactory.getService(
          p,
          effectiveApiKey,
          resolved.serviceConfig,
        );
        const modelList = await withTimeout(service.listModels(), 20000);

        const modelsRecord = modelList.reduce(
          (acc, modelInfo) => {
            const k = modelInfo.id || modelInfo.name;
            acc[k] = modelInfo;
            return acc;
          },
          {} as Record<string, ModelInfo>,
        );

        if (Object.keys(modelsRecord).length > 0) {
          setStoredModelCache(p, modelsRecord);
        }

        return modelsRecord;
      } catch (error) {
        logger.error('Failed to fetch models locally:', error);
        const storedModels = getStoredModelCache(p);
        const hasCachedModels = !!(
          storedModels && Object.keys(storedModels).length > 0
        );
        reportListModelsFallback({
          provider: p,
          baseUrl: resolved.baseUrl,
          reason: 'api_error',
          error,
          hasCachedModels,
        });
        return storedModels || {};
      }
    },
    [resolved],
  );

  const {
    data: dynamicModels = {},
    mutate: mutateModels,
    isValidating: isRefreshing,
  } = useSWR<DynamicModelMap>(swrKey, fetchDynamicModels, {
    revalidateOnFocus: false,
    keepPreviousData: true,
    dedupingInterval: 30000,
  });

  const refreshModels = useCallback(async () => {
    if (!dynamicModelPolicy.canFetch || !resolved || !provider) {
      return dynamicModels;
    }

    AIServiceFactory.invalidateService(
      provider,
      apiKey,
      resolved.serviceConfig,
    );

    return await mutateModels(undefined, {
      revalidate: true,
    });
  }, [
    apiKey,
    dynamicModelPolicy.canFetch,
    dynamicModels,
    mutateModels,
    provider,
    resolved,
  ]);

  const availableModels = useMemo(() => {
    if (!provider || !resolved) return {};

    return resolveProviderModels(
      provider,
      { serviceConfigs, customProviders },
      dynamicModels,
    );
  }, [provider, dynamicModels, resolved, serviceConfigs, customProviders]);

  return {
    availableModels,
    isRefreshing,
    refreshModels,
    canRefresh: dynamicModelPolicy.canFetch,
    refreshBlockedReason: dynamicModelPolicy.reason,
  };
};
