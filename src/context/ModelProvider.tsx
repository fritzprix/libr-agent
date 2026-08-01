import {
  createContext,
  useCallback,
  useMemo,
  useContext,
  FC,
  PropsWithChildren,
} from 'react';
import useSWR from 'swr';
import { withTimeout } from '../lib/retry-utils';
import {
  AIServiceProvider,
  AIServiceFactory,
  isCustomOpenAIProviderId,
  listCustomProviderPickerOptions,
  resolveProviderRuntimeConfig,
} from '../lib/ai-service';
import { shouldFetchDynamicModels } from '../lib/ai-service/model-fetch-policy';
import {
  llmConfigManager,
  ModelInfo,
  ProviderInfo,
} from '../lib/llm-config-manager';
import { useSettings } from '../hooks/use-settings';
import { getLogger } from '@/lib/logger';
import { reportListModelsFallback } from '@/lib/ai-service/list-models-errors';
import {
  getStoredModelCache,
  setStoredModelCache,
} from '@/lib/ai-service/model-cache-storage';

const DEFAULT_MODEL_INFO: ModelInfo = {
  contextWindow: 0,
  supportTools: false,
  supportReasoning: false,
  supportStreaming: false,
  cost: { input: 0, output: 0 },
  description: '',
  name: '',
};

const logger = getLogger('ModelProvider');

interface ModelOptionsContextType {
  modelId: string;
  /** Builtin provider id or `custom:<id>`. */
  provider: string;
  models: Record<string, ModelInfo>;
  providers: Array<ProviderInfo>;
  currentProviderInfo: ProviderInfo | null;
  setProvider: (provider: string) => void;
  setModel: (modelId: string) => void;
  isLoading: boolean;
  apiKeys: Record<AIServiceProvider, string>;
  selectedModelData: ModelInfo;
  providerOptions: { label: string; value: string }[];
  modelOptions: { label: string; value: string }[];
  refreshModels: () => Promise<void>;
  isRefreshingModels: boolean;
}

const ModelOptionsContext = createContext<ModelOptionsContextType | null>(null);

export const ModelOptionsProvider: FC<PropsWithChildren> = ({ children }) => {
  const {
    value: {
      serviceConfigs,
      customProviders,
      preferredModel: { model, provider },
    },
    update,
    isLoading,
  } = useSettings();

  const resolvedProvider = useMemo(
    () =>
      resolveProviderRuntimeConfig(provider, {
        serviceConfigs,
        customProviders,
      }),
    [provider, serviceConfigs, customProviders],
  );

  // Builtin-provider API keys only. Custom (`custom:<id>`) keys are resolved via
  // resolveProviderRuntimeConfig() — do not look up custom ids in this map.
  const apiKeys = useMemo(() => {
    return Object.entries(serviceConfigs).reduce(
      (acc, [providerKey, config]) => {
        acc[providerKey as AIServiceProvider] = config.apiKey || '';
        return acc;
      },
      {} as Record<AIServiceProvider, string>,
    );
  }, [serviceConfigs]);

  // Generate stable cache key including baseUrl
  const swrCacheKey = useMemo(() => {
    if (
      !shouldFetchDynamicModels({
        provider,
        apiKey: resolvedProvider.apiKey,
        baseUrl: resolvedProvider.baseUrl,
        use3rdParty: resolvedProvider.use3rdParty,
        customModelId: resolvedProvider.customModelId,
      })
    ) {
      return null;
    }

    return [
      'models',
      provider,
      resolvedProvider.apiKey || '',
      resolvedProvider.baseUrl || '',
      resolvedProvider.use3rdParty ? 'use-3rd-party' : 'first-party',
      resolvedProvider.customModelId || '',
    ];
  }, [provider, resolvedProvider]);

  // Fetcher for models — always delegates to service.listModels(), which each
  // provider implements. Static-only providers return llmConfigManager data;
  // dynamic providers (Ollama, OpenAI, OpenRouter, …) fetch from their APIs.
  const fetchDynamicModels = useCallback(
    async ([, providerId, apiKey]: [string, string, string]) => {
      const effectiveApiKey = apiKey || 'no-api-key';
      const resolved = resolveProviderRuntimeConfig(providerId, {
        serviceConfigs,
        customProviders,
      });

      try {
        const service = AIServiceFactory.getService(
          providerId,
          effectiveApiKey,
          resolved.serviceConfig,
        );
        const modelList = await withTimeout(service.listModels(), 20000);

        const modelsRecord = Object.create(null) as Record<string, ModelInfo>;
        for (const modelInfo of modelList) {
          const key = modelInfo.id || modelInfo.name;
          modelsRecord[key] = modelInfo;
        }

        if (Object.keys(modelsRecord).length > 0) {
          setStoredModelCache(providerId, modelsRecord);
        }

        logger.info(`Fetched ${modelList.length} models from ${providerId}`);
        return modelsRecord;
      } catch (error) {
        logger.error('Failed to fetch models:', error);
        const storedModels = getStoredModelCache(providerId);
        const hasCachedModels = !!(
          storedModels && Object.keys(storedModels).length > 0
        );
        reportListModelsFallback({
          provider: providerId,
          baseUrl: resolved.baseUrl,
          reason: 'api_error',
          error,
          hasCachedModels,
        });
        return storedModels || {};
      }
    },
    [serviceConfigs, customProviders],
  );

  // SWR for dynamic models
  const {
    data: dynamicModels = {},
    mutate: mutateModels,
    isValidating: isRefreshingModels,
  } = useSWR(swrCacheKey, fetchDynamicModels, {
    revalidateOnFocus: false,
    staleWhileRevalidate: true,
    keepPreviousData: true,
    dedupingInterval: 30000, // 30 seconds
  });

  // Prefer dynamic list, then persisted cache, then static config / manual models.
  const models = useMemo(() => {
    const staticModels = isCustomOpenAIProviderId(provider)
      ? {}
      : llmConfigManager.getModelsForProvider(provider as AIServiceProvider) ||
        {};
    const storedModels = getStoredModelCache(provider);
    const manualModels = (resolvedProvider.manualModels ?? []).reduce<
      Record<string, ModelInfo>
    >((acc, modelId) => {
      acc[modelId] = {
        ...DEFAULT_MODEL_INFO,
        id: modelId,
        name: modelId,
        contextWindow: 128000,
        supportTools: true,
        supportStreaming: true,
        description: 'Custom OpenAI-compatible model',
      };
      return acc;
    }, {});

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
  }, [provider, dynamicModels, resolvedProvider.manualModels]);

  const refreshModels = useCallback(async () => {
    AIServiceFactory.invalidateService(
      provider,
      resolvedProvider.apiKey || '',
      resolvedProvider.serviceConfig,
    );
    await mutateModels(undefined, {
      revalidate: true,
    });
  }, [mutateModels, provider, resolvedProvider]);

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();
    const builtin = Object.entries(providers).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
    return [...builtin, ...listCustomProviderPickerOptions(customProviders)];
  }, [customProviders]);

  const modelOptions = useMemo(() => {
    logger.info('🎯 Current provider:', provider);
    logger.info('📦 Models for provider:', models);

    const options = Object.entries(models).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));

    logger.info('🔄 Generated modelOptions:', options);
    return options;
  }, [models, provider]);

  const selectedModelData = useMemo(() => {
    return models[model] || DEFAULT_MODEL_INFO;
  }, [models, model]);

  const currentProviderInfo = useMemo(() => {
    if (isCustomOpenAIProviderId(provider)) {
      return {
        name: resolvedProvider.displayName,
        apiKeyEnvVar: '',
        baseUrl: resolvedProvider.baseUrl ?? '',
        requiresApiKey: false,
        models: {},
      } satisfies ProviderInfo;
    }
    const providers = llmConfigManager.getProviders();
    return providers[provider] || null;
  }, [provider, resolvedProvider.baseUrl, resolvedProvider.displayName]);

  const setProvider = useCallback(
    (newProvider: string) => {
      if (isCustomOpenAIProviderId(newProvider)) {
        const resolved = resolveProviderRuntimeConfig(newProvider, {
          serviceConfigs,
          customProviders,
        });
        const defaultModel = resolved.manualModels?.[0] ?? '';
        update({
          preferredModel: { provider: newProvider, model: defaultModel },
        });
        return;
      }

      const availableModels =
        llmConfigManager.getModelsForProvider(
          newProvider as AIServiceProvider,
        ) || {};

      if (Object.keys(availableModels).length === 0) {
        logger.warn(`No available models for ${newProvider}`);
        update({ preferredModel: { provider: newProvider, model: '' } });
        return;
      }

      const modelEntries = Object.entries(availableModels);
      const newModel = modelEntries.length > 0 ? modelEntries[0][0] : '';

      update({ preferredModel: { provider: newProvider, model: newModel } });
    },
    [update, serviceConfigs, customProviders],
  );

  const setModel = useCallback(
    (newModel: string) => {
      update({ preferredModel: { provider, model: newModel } });
    },
    [provider, update],
  );

  const contextValue = useMemo(
    () => ({
      modelId: model,
      provider,
      models,
      providers: Object.values(llmConfigManager.getProviders()),
      currentProviderInfo,
      setProvider,
      setModel,
      isLoading: isLoading || isRefreshingModels,
      apiKeys,
      selectedModelData,
      providerOptions,
      modelOptions,
      refreshModels,
      isRefreshingModels,
    }),
    [
      model,
      provider,
      models,
      currentProviderInfo,
      setProvider,
      setModel,
      isLoading,
      isRefreshingModels,
      apiKeys,
      selectedModelData,
      providerOptions,
      modelOptions,
      refreshModels,
    ],
  );

  return (
    <ModelOptionsContext.Provider value={contextValue}>
      {children}
    </ModelOptionsContext.Provider>
  );
};

export const useModelOptions = () => {
  const context = useContext(ModelOptionsContext);
  if (!context) {
    throw new Error(
      'useModelOptions must be used within a ModelOptionsProvider',
    );
  }
  return context;
};
