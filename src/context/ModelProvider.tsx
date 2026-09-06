import {
  createContext,
  useCallback,
  useMemo,
  useContext,
  FC,
  PropsWithChildren,
} from 'react';
import useSWR from 'swr';
import {
  AIServiceProvider,
  AIServiceFactory,
  isCustomOpenAIProviderId,
  listCustomProviderPickerOptions,
  resolveDefaultModelForProviderChange,
  resolveProviderRuntimeConfig,
} from '../lib/ai-service';
import { setLastSelectedModel } from '../lib/ai-service/last-selected-model-storage';
import { shouldFetchDynamicModels } from '../lib/ai-service/model-fetch-policy';
import {
  BACKGROUND_LIST_MODELS_TIMEOUT_MS,
  REFRESH_LIST_MODELS_TIMEOUT_MS,
  buildProviderModelsSwrSegment,
  fetchDynamicProviderModels,
} from '../lib/ai-service/fetch-dynamic-provider-models';
import {
  llmConfigManager,
  ModelInfo,
  ProviderInfo,
} from '../lib/llm-config-manager';
import { useSettings } from '../hooks/use-settings';
import { getLogger } from '@/lib/logger';
import { getStoredModelCache } from '@/lib/ai-service/model-cache-storage';

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
    value: { serviceConfigs, customProviders, preferredModel, fallbackModel },
    update,
    isLoading,
  } = useSettings();
  const { model, provider } = preferredModel;

  const settingsSlice = useMemo(
    () => ({ serviceConfigs, customProviders }),
    [serviceConfigs, customProviders],
  );

  const resolvedProvider = useMemo(
    () => resolveProviderRuntimeConfig(provider, settingsSlice),
    [provider, settingsSlice],
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

  // SWR key fingerprints the API key — fetcher resolves credentials from settings.
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
      buildProviderModelsSwrSegment({
        providerId: provider,
        apiKey: resolvedProvider.apiKey,
        baseUrl: resolvedProvider.baseUrl,
        use3rdParty: resolvedProvider.use3rdParty,
        customModelId: resolvedProvider.customModelId,
      }),
    ] as const;
  }, [provider, resolvedProvider]);

  const fetchDynamicModels = useCallback(
    async ([, providerSegment]: readonly ['models', string]) => {
      const providerId = providerSegment.split('|')[0] ?? provider;
      return fetchDynamicProviderModels(providerId, settingsSlice, {
        timeoutMs: BACKGROUND_LIST_MODELS_TIMEOUT_MS,
        notifyUser: false,
      });
    },
    [provider, settingsSlice],
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
    const nextModels = await fetchDynamicProviderModels(
      provider,
      settingsSlice,
      {
        timeoutMs: REFRESH_LIST_MODELS_TIMEOUT_MS,
        notifyUser: true,
      },
    );
    await mutateModels(nextModels, { revalidate: false });
  }, [mutateModels, provider, resolvedProvider, settingsSlice]);

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();
    const builtin = Object.entries(providers).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
    return [...builtin, ...listCustomProviderPickerOptions(customProviders)];
  }, [customProviders]);

  const modelOptions = useMemo(() => {
    return Object.entries(models).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, [models]);

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
      const defaultModel = resolveDefaultModelForProviderChange(
        newProvider,
        {
          serviceConfigs,
          customProviders,
          preferredModel,
          fallbackModel,
        },
        model,
      );
      if (!defaultModel) {
        logger.warn(`No available models for ${newProvider}`);
      } else {
        setLastSelectedModel(newProvider, defaultModel);
      }
      update({
        preferredModel: { provider: newProvider, model: defaultModel },
      });
    },
    [
      update,
      serviceConfigs,
      customProviders,
      preferredModel,
      fallbackModel,
      model,
    ],
  );

  const setModel = useCallback(
    (newModel: string) => {
      if (provider && newModel) {
        setLastSelectedModel(provider, newModel);
      }
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
