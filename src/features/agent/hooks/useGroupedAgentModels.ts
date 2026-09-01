import { useCallback, useMemo } from 'react';
import useSWR from 'swr';

import { useSettings } from '@/hooks/use-settings';
import {
  AIServiceFactory,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service';
import {
  buildSettingsSnapshot,
  listConfiguredProviderGroups,
  type ProviderPickerGroup,
} from '@/lib/ai-service/configured-providers';
import { getDynamicModelFetchPolicy } from '@/lib/ai-service/model-fetch-policy';
import {
  resolveProviderModels,
  type ProviderModelMap,
} from '@/lib/ai-service/resolve-provider-models';
import type { AIModelLookupService } from '@/lib/ai-service/types';
import type { ModelInfo } from '@/lib/llm-config-manager';
import { withTimeout } from '@/lib/retry-utils';
import { getLogger } from '@/lib/logger';
import { reportListModelsFallback } from '@/lib/ai-service/list-models-errors';
import { setStoredModelCache } from '@/lib/ai-service/model-cache-storage';
import type {
  CustomOpenAIProvider,
  Settings,
} from '@/lib/services/settings-service';

const logger = getLogger('useGroupedAgentModels');

export interface GroupedProviderModels {
  providerId: string;
  label: string;
  models: ProviderModelMap;
}

type GroupedDynamicModelMap = Record<string, ProviderModelMap>;
type GroupedModelsSWRKey = readonly ['grouped-models', string, ...string[]];

interface UseGroupedAgentModelsOptions {
  customProviders?: CustomOpenAIProvider[];
  serviceConfigs?: Settings['serviceConfigs'];
  /** Provider used for contextual refresh controls. */
  currentProvider?: string;
}

async function fetchDynamicModelsForProvider(
  providerId: string,
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
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

  const effectiveApiKey = resolved.apiKey || 'no-api-key';

  try {
    const service: AIModelLookupService = AIServiceFactory.getService(
      providerId,
      effectiveApiKey,
      resolved.serviceConfig,
    );
    const modelList = await withTimeout(service.listModels(), 20000);

    const modelsRecord = modelList.reduce<ProviderModelMap>(
      (acc, modelInfo) => {
        const key = modelInfo.id || modelInfo.name;
        acc[key] = modelInfo;
        return acc;
      },
      {},
    );

    if (Object.keys(modelsRecord).length > 0) {
      setStoredModelCache(providerId, modelsRecord);
    }

    return modelsRecord;
  } catch (error) {
    logger.error(`Failed to fetch models for ${providerId}:`, error);
    const storedModels = resolveProviderModels(providerId, settings);
    reportListModelsFallback({
      provider: providerId,
      baseUrl: resolved.baseUrl,
      reason: 'api_error',
      error,
      hasCachedModels: Object.keys(storedModels).length > 0,
    });
    return {};
  }
}

function buildGroupedSWRKey(
  settings: Pick<Settings, 'serviceConfigs' | 'customProviders'>,
  providerGroups: ProviderPickerGroup[],
): GroupedModelsSWRKey | null {
  if (providerGroups.length === 0) {
    return null;
  }

  const providerEntries = providerGroups
    .map((group) => {
      const resolved = resolveProviderRuntimeConfig(group.providerId, settings);
      const policy = getDynamicModelFetchPolicy({
        provider: group.providerId,
        apiKey: resolved.apiKey,
        baseUrl: resolved.baseUrl,
        use3rdParty: resolved.use3rdParty,
        customModelId: resolved.customModelId,
      });

      if (!policy.canFetch) {
        return null;
      }

      return [
        group.providerId,
        resolved.apiKey,
        resolved.baseUrl ?? '',
        resolved.use3rdParty ? 'use-3rd-party' : 'first-party',
        resolved.customModelId ?? '',
      ].join('|');
    })
    .filter((entry): entry is string => entry !== null)
    .sort();

  if (providerEntries.length === 0) {
    return null;
  }

  return ['grouped-models', providerEntries.join(';;')] as const;
}

export const useGroupedAgentModels = ({
  customProviders: customProvidersProp,
  serviceConfigs: serviceConfigsProp,
  currentProvider,
}: UseGroupedAgentModelsOptions = {}) => {
  const {
    value: { serviceConfigs: settingsServiceConfigs, customProviders },
  } = useSettings();

  const settings = useMemo(
    () =>
      buildSettingsSnapshot(
        serviceConfigsProp ?? settingsServiceConfigs,
        customProvidersProp ?? customProviders,
      ),
    [
      customProviders,
      customProvidersProp,
      serviceConfigsProp,
      settingsServiceConfigs,
    ],
  );

  const providerGroups = useMemo(
    () => listConfiguredProviderGroups(settings),
    [settings],
  );

  const swrKey = useMemo(
    () => buildGroupedSWRKey(settings, providerGroups),
    [providerGroups, settings],
  );

  const fetchGroupedDynamicModels = useCallback(
    async ([, providerSignature]: GroupedModelsSWRKey) => {
      const providerIds = providerSignature
        .split(';;')
        .map((entry) => entry.split('|')[0])
        .filter((providerId): providerId is string => Boolean(providerId));

      const entries = await Promise.all(
        providerIds.map(async (providerId) => {
          const dynamicModels = await fetchDynamicModelsForProvider(
            providerId,
            settings,
          );
          return [providerId, dynamicModels] as const;
        }),
      );

      return Object.fromEntries(entries) as GroupedDynamicModelMap;
    },
    [settings],
  );

  const {
    data: dynamicModelsByProvider = {},
    mutate: mutateGroupedModels,
    isValidating: isRefreshingGrouped,
  } = useSWR<GroupedDynamicModelMap>(swrKey, fetchGroupedDynamicModels, {
    revalidateOnFocus: false,
    keepPreviousData: true,
    dedupingInterval: 30000,
  });

  const groupedModels = useMemo<GroupedProviderModels[]>(() => {
    return providerGroups.map((group) => ({
      providerId: group.providerId,
      label: group.label,
      models: resolveProviderModels(
        group.providerId,
        settings,
        dynamicModelsByProvider[group.providerId],
      ),
    }));
  }, [dynamicModelsByProvider, providerGroups, settings]);

  const currentProviderPolicy = useMemo(() => {
    if (!currentProvider) {
      return {
        canFetch: false,
        reason: 'missing-provider' as const,
      };
    }

    const resolved = resolveProviderRuntimeConfig(currentProvider, settings);
    return getDynamicModelFetchPolicy({
      provider: currentProvider,
      apiKey: resolved.apiKey,
      baseUrl: resolved.baseUrl,
      use3rdParty: resolved.use3rdParty,
      customModelId: resolved.customModelId,
    });
  }, [currentProvider, settings]);

  const refreshModels = useCallback(async () => {
    if (!currentProvider || !currentProviderPolicy.canFetch) {
      return;
    }

    const resolved = resolveProviderRuntimeConfig(currentProvider, settings);
    AIServiceFactory.invalidateService(
      currentProvider,
      resolved.apiKey,
      resolved.serviceConfig,
    );

    await mutateGroupedModels(
      async (current) => {
        const dynamicModels = await fetchDynamicModelsForProvider(
          currentProvider,
          settings,
        );
        return {
          ...current,
          [currentProvider]: dynamicModels,
        };
      },
      { revalidate: false },
    );
  }, [
    currentProvider,
    currentProviderPolicy.canFetch,
    mutateGroupedModels,
    settings,
  ]);

  const getModelInfo = useCallback(
    (providerId: string, modelId: string): ModelInfo | undefined => {
      const group = groupedModels.find(
        (entry) => entry.providerId === providerId,
      );
      return group?.models[modelId];
    },
    [groupedModels],
  );

  return {
    groupedModels,
    hasConfiguredProviders: providerGroups.length > 0,
    isRefreshing: isRefreshingGrouped,
    refreshModels,
    canRefresh: currentProviderPolicy.canFetch,
    refreshBlockedReason: currentProviderPolicy.reason,
    getModelInfo,
  };
};
