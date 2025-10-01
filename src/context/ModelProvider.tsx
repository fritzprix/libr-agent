import {
  createContext,
  useCallback,
  useMemo,
  useContext,
  FC,
  PropsWithChildren,
  useEffect,
  useState,
} from 'react';
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
  const [dynamicModels, setDynamicModels] = useState<
    Record<string, Record<string, ModelInfo>>
  >({});
  const [isRefreshingModels, setIsRefreshingModels] = useState(false);

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

  // 현재 프로바이더의 모델 목록 계산
  const models = useMemo(() => {
    const staticModels = llmConfigManager.getModelsForProvider(provider) || {};

    // 동적 목록이 provider별로 있으면 우선 사용
    const dynForProvider = dynamicModels[provider] || {};
    if (Object.keys(dynForProvider).length > 0) {
      return dynForProvider;
    }

    return staticModels;
  }, [provider, dynamicModels]);

  // 수동 모델 새로고침 함수 (새로고침 버튼용)
  const refreshModels = useCallback(async () => {
    // Only attempt dynamic listing for providers that support it
    const supportsDynamic =
      provider === AIServiceProvider.Ollama ||
      provider === AIServiceProvider.OpenAI;

    if (!supportsDynamic) return;

    // For Ollama we can instantiate even without a real API key; for others require a key
    let apiKey = apiKeys[provider];
    if (!apiKey) {
      if (provider === AIServiceProvider.Ollama) {
        logger.info(
          'No API key configured for Ollama — using dummy key to instantiate service',
        );
        apiKey = 'ollama-dummy';
      } else {
        logger.warn(
          `No API key available for ${provider}, skipping model refresh`,
        );
        return;
      }
    }

    setIsRefreshingModels(true);
    try {
      const service = AIServiceFactory.getService(provider, apiKey);
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

      setDynamicModels((prev) => ({ ...prev, [provider]: modelsRecord }));
      logger.info(
        `Manually refreshed ${modelList.length} models from ${provider}`,
      );
    } catch (error) {
      logger.error('Failed to manually refresh models:', error);
      // 에러 시 provider별 동적 목록을 비워서 정적 JSON으로 fallback
      setDynamicModels((prev) => ({ ...prev, [provider]: {} }));
    } finally {
      setIsRefreshingModels(false);
    }
  }, [provider, apiKeys]);

  // 프로바이더가 Ollama로 변경될 때 자동으로 모델 목록 새로고침
  useEffect(() => {
    const fetchOllamaModels = async () => {
      // For providers that support dynamic listing (Ollama, OpenAI), attempt to fetch models.
      const supportsDynamic =
        provider === AIServiceProvider.Ollama ||
        provider === AIServiceProvider.OpenAI;

      if (!supportsDynamic) {
        // Ensure we clear any previously stored dynamic models for this provider
        setDynamicModels((prev) => ({ ...prev, [provider]: {} }));
        return;
      }

      // Decide API key availability. Ollama can use dummy key; OpenAI requires real key.
      let apiKey = apiKeys[provider];
      if (!apiKey) {
        if (provider === AIServiceProvider.Ollama) {
          logger.info(
            'No API key configured for Ollama in settings — using dummy key to instantiate service',
          );
          apiKey = 'ollama-dummy';
        } else {
          logger.warn(
            `No API key configured for ${provider}, skipping automatic model fetch`,
          );
          // Clear any dynamic models for this provider so static JSON is used
          setDynamicModels((prev) => ({ ...prev, [provider]: {} }));
          return;
        }
      }

      setIsRefreshingModels(true);
      try {
        const service = AIServiceFactory.getService(provider, apiKey);
        const modelList = await service.listModels();

        const modelsRecord = modelList.reduce(
          (acc, modelInfo) => {
            const key = modelInfo.id || modelInfo.name;
            acc[key] = modelInfo;
            return acc;
          },
          {} as Record<string, ModelInfo>,
        );

        setDynamicModels((prev) => ({ ...prev, [provider]: modelsRecord }));
        logger.info(
          `Auto-refreshed ${modelList.length} models from ${provider}`,
        );
      } catch (error) {
        logger.error('Failed to auto-refresh models:', error);
        setDynamicModels((prev) => ({ ...prev, [provider]: {} }));
      } finally {
        setIsRefreshingModels(false);
      }
    };

    fetchOllamaModels();
  }, [provider, apiKeys]); // 정확한 의존성만 포함

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
      let availableModels: Record<string, ModelInfo> = {};

      const dynForProvider = dynamicModels[newProvider] || {};

      if (Object.keys(dynForProvider).length > 0) {
        availableModels = dynForProvider;
      } else {
        availableModels =
          llmConfigManager.getModelsForProvider(newProvider) || {};
      }

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
    [update, dynamicModels],
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
