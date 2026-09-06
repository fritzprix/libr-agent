import { useCallback, useMemo } from 'react';
import useSWR from 'swr';

import { useDebouncedValue } from '@/features/knowledge/hooks/useDebouncedValue';
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
import {
  BACKGROUND_LIST_MODELS_TIMEOUT_MS,
  REFRESH_LIST_MODELS_TIMEOUT_MS,
  buildProviderModelsSwrSegment,
  fetchDynamicProviderModels,
} from '@/lib/ai-service/fetch-dynamic-provider-models';
import { getDynamicModelFetchPolicy } from '@/lib/ai-service/model-fetch-policy';
import {
  resolveProviderModels,
  type ProviderModelMap,
} from '@/lib/ai-service/resolve-provider-models';
import type { ModelInfo } from '@/lib/llm-config-manager';
import { getLogger } from '@/lib/logger';
import type {
  CustomOpenAIProvider,
  Settings,
} from '@/lib/services/settings-service';

const logger = getLogger('useGroupedAgentModels');

/** Stabilize draft credential keystrokes before changing the SWR key. */
const FETCH_SETTINGS_DEBOUNCE_MS = 800;

export interface GroupedProviderModels {
  providerId: string;
  label: string;
  models: ProviderModelMap;
}

type GroupedDynamicModelMap = Record<string, ProviderModelMap>;
type GroupedModelsSWRKey = readonly ['grouped-models', string, ...string[]];
type SettingsSlice = Pick<Settings, 'serviceConfigs' | 'customProviders'>;

interface UseGroupedAgentModelsOptions {
  customProviders?: CustomOpenAIProvider[];
  serviceConfigs?: Settings['serviceConfigs'];
  /** Provider used for contextual refresh controls. */
  currentProvider?: string;
}

function buildGroupedSWRKey(
  settings: SettingsSlice,
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

      return buildProviderModelsSwrSegment({
        providerId: group.providerId,
        apiKey: resolved.apiKey,
        baseUrl: resolved.baseUrl,
        use3rdParty: resolved.use3rdParty,
        customModelId: resolved.customModelId,
      });
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

  const displaySettings = useMemo(
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

  // Draft keystrokes update display immediately; SWR key waits for quiet period.
  const fetchSettings = useDebouncedValue(
    displaySettings,
    FETCH_SETTINGS_DEBOUNCE_MS,
  );

  const providerGroups = useMemo(
    () => listConfiguredProviderGroups(displaySettings),
    [displaySettings],
  );

  const fetchProviderGroups = useMemo(
    () => listConfiguredProviderGroups(fetchSettings),
    [fetchSettings],
  );

  const swrKey = useMemo(
    () => buildGroupedSWRKey(fetchSettings, fetchProviderGroups),
    [fetchProviderGroups, fetchSettings],
  );

  const fetchGroupedDynamicModels = useCallback(
    async ([, providerSignature]: GroupedModelsSWRKey) => {
      const providerIds = providerSignature
        .split(';;')
        .map((entry) => entry.split('|')[0])
        .filter((providerId): providerId is string => Boolean(providerId));

      const settled = await Promise.allSettled(
        providerIds.map(async (providerId) => {
          const dynamicModels = await fetchDynamicProviderModels(
            providerId,
            fetchSettings,
            {
              timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
              notifyUser: false,
            },
          );
          return [providerId, dynamicModels] as const;
        }),
      );

      const entries = settled.map((result, index) => {
        if (result.status === 'fulfilled') {
          return result.value;
        }
        const providerId = providerIds[index] ?? `unknown-${index}`;
        const reasonMessage =
          result.reason instanceof Error
            ? result.reason.message
            : String(result.reason);
        logger.warn(
          `Grouped model fetch rejected for ${providerId}:`,
          reasonMessage,
        );
        return [providerId, {}] as const;
      });

      return Object.fromEntries(entries) as GroupedDynamicModelMap;
    },
    [fetchSettings],
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
        displaySettings,
        dynamicModelsByProvider[group.providerId],
      ),
    }));
  }, [displaySettings, dynamicModelsByProvider, providerGroups]);

  const currentProviderPolicy = useMemo(() => {
    if (!currentProvider) {
      return {
        canFetch: false,
        reason: 'missing-provider' as const,
      };
    }

    const resolved = resolveProviderRuntimeConfig(
      currentProvider,
      displaySettings,
    );
    return getDynamicModelFetchPolicy({
      provider: currentProvider,
      apiKey: resolved.apiKey,
      baseUrl: resolved.baseUrl,
      use3rdParty: resolved.use3rdParty,
      customModelId: resolved.customModelId,
    });
  }, [currentProvider, displaySettings]);

  const refreshModels = useCallback(async () => {
    if (!currentProvider || !currentProviderPolicy.canFetch) {
      return;
    }

    const resolved = resolveProviderRuntimeConfig(
      currentProvider,
      displaySettings,
    );
    AIServiceFactory.invalidateService(
      currentProvider,
      resolved.apiKey,
      resolved.serviceConfig,
    );

    await mutateGroupedModels(
      async (current) => {
        const dynamicModels = await fetchDynamicProviderModels(
          currentProvider,
          displaySettings,
          {
            timeoutMs: REFRESH_LIST_MODELS_TIMEOUT_MS,
            notifyUser: true,
          },
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
    displaySettings,
    mutateGroupedModels,
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
