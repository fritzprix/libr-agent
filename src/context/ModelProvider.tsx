import {
  createContext,
  useCallback,
  useMemo,
  useContext,
  FC,
  PropsWithChildren,
} from 'react';
import useSWR from 'swr';
import { AIServiceProvider, AIServiceFactory } from '../lib/ai-service';
import {
  llmConfigManager,
  ModelInfo,
  ProviderInfo,
} from '../lib/llm-config-manager';
import { useSettings } from '../hooks/use-settings';
import { getLogger } from '@/lib/logger';

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
  provider: AIServiceProvider;
  models: Record<string, ModelInfo>;
  providers: Array<ProviderInfo>;
  setProvider: (provider: AIServiceProvider) => void;
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
      preferredModel: { model, provider },
    },
    update,
    isLoading,
  } = useSettings();

  // 동적 모델 목록 상태 (provider별로 저장)
  // const [isRefreshingModels, setIsRefreshingModels] = useState(false); // Removed, using SWR isValidating

  // Compute API keys from service configs for backward compatibility
  const apiKeys = useMemo(() => {
    return Object.entries(serviceConfigs).reduce(
      (acc, [provider, config]) => {
        acc[provider as AIServiceProvider] = config.apiKey || '';
        return acc;
      },
      {} as Record<AIServiceProvider, string>,
    );
  }, [serviceConfigs]);

  // Fetcher for dynamic models
  const fetchDynamicModels = useCallback(
    async ([, provider, apiKey]: [string, string, string]) => {
      const supportsDynamic =
        provider === AIServiceProvider.Ollama ||
        provider === AIServiceProvider.OpenAI ||
        provider === AIServiceProvider.Anthropic;

      if (!supportsDynamic) return {};

      let effectiveApiKey = apiKey;
      if (!effectiveApiKey) {
        if (provider === AIServiceProvider.Ollama) {
          logger.info(
            'No API key configured for Ollama — using dummy key to instantiate service',
          );
          effectiveApiKey = 'ollama-dummy';
        } else {
          logger.warn(
            `No API key available for ${provider}, skipping model fetch`,
          );
          return {};
        }
      }

      try {
        const service = AIServiceFactory.getService(
          provider as AIServiceProvider,
          effectiveApiKey,
        );
        const modelList = await service.listModels();

        // Convert ModelInfo[] into Record<string, ModelInfo>
        const modelsRecord = modelList.reduce(
          (acc, modelInfo) => {
            const key = modelInfo.id || modelInfo.name;
            acc[key] = modelInfo;
            return acc;
          },
          {} as Record<string, ModelInfo>,
        );

        logger.info(`Fetched ${modelList.length} models from ${provider}`);
        return modelsRecord;
      } catch (error) {
        logger.error('Failed to fetch models:', error);
        return {};
      }
    },
    [],
  );

  // SWR for dynamic models
  const {
    data: dynamicModels = {},
    mutate: mutateModels,
    isValidating: isRefreshingModels,
  } = useSWR(['models', provider, apiKeys[provider]], fetchDynamicModels, {
    revalidateOnFocus: false,
    staleWhileRevalidate: true,
    dedupingInterval: 30000, // 30 seconds
  });

  // 현재 프로바이더의 모델 목록 계산
  const models = useMemo(() => {
    const staticModels = llmConfigManager.getModelsForProvider(provider) || {};

    // 동적 목록이 provider별로 있으면 우선 사용
    if (Object.keys(dynamicModels).length > 0) {
      return dynamicModels;
    }

    return staticModels;
  }, [provider, dynamicModels]);

  // 수동 모델 새로고침 함수 (새로고침 버튼용)
  const refreshModels = useCallback(async () => {
    // Only attempt dynamic listing for providers that support it
    const supportsDynamic =
      provider === AIServiceProvider.Ollama ||
      provider === AIServiceProvider.OpenAI ||
      provider === AIServiceProvider.Anthropic;

    if (!supportsDynamic) return;

    // Trigger revalidation via SWR mutate
    await mutateModels();
  }, [provider, mutateModels]);

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();

    return Object.entries(providers).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, []);

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

  const setProvider = useCallback(
    (newProvider: AIServiceProvider) => {
      const availableModels =
        llmConfigManager.getModelsForProvider(newProvider) || {};

      if (Object.keys(availableModels).length === 0) {
        logger.warn(`No available models for ${newProvider}`);
        // 모델이 없어도 프로바이더는 변경하고, 모델은 빈 문자열로 설정
        update({ preferredModel: { provider: newProvider, model: '' } });
        return;
      }

      const modelEntries = Object.entries(availableModels);
      const newModel = modelEntries.length > 0 ? modelEntries[0][0] : '';

      update({ preferredModel: { provider: newProvider, model: newModel } });
    },
    [update],
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
