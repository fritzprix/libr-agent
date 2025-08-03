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

const logger = getLogger("ModelProvider");

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
      apiKeys,
      preferredModel: { model, provider },
    },
    update,
    isLoading,
  } = useSettings();

  // 동적 모델 목록 상태 (주로 Ollama용)
  const [dynamicModels, setDynamicModels] = useState<Record<string, ModelInfo>>(
    {},
  );
  const [isRefreshingModels, setIsRefreshingModels] = useState(false);

  // 현재 프로바이더의 모델 목록 계산
  const models = useMemo(() => {
    const staticModels = llmConfigManager.getModelsForProvider(provider) || {};

    // Ollama의 경우 동적 모델 목록 우선 사용
    if (
      provider === AIServiceProvider.Ollama &&
      Object.keys(dynamicModels).length > 0
    ) {
      return dynamicModels;
    }

    return staticModels;
  }, [provider, dynamicModels]);

  // 수동 모델 새로고침 함수 (새로고침 버튼용)
  const refreshModels = useCallback(async () => {
    // Ollama가 아니면 새로고침 불필요
    if (provider !== AIServiceProvider.Ollama) {
      return;
    }

    const apiKey = apiKeys[provider];
    if (!apiKey) {
      logger.warn('No API key available for Ollama');
      return;
    }

    setIsRefreshingModels(true);
    try {
      const service = AIServiceFactory.getService(provider, apiKey);
      const modelList = await service.listModels();

      // ModelInfo[] 배열을 Record<string, ModelInfo>로 변환
      const modelsRecord = modelList.reduce(
        (acc, modelInfo) => {
          const key = modelInfo.id || modelInfo.name;
          acc[key] = modelInfo;
          return acc;
        },
        {} as Record<string, ModelInfo>,
      );

      setDynamicModels(modelsRecord);
      logger.info(`Manually refreshed ${modelList.length} models from Ollama server`);
    } catch (error) {
      logger.error('Failed to manually refresh models:', error);
      // 에러 시 빈 객체로 설정하여 정적 모델로 fallback
      setDynamicModels({});
    } finally {
      setIsRefreshingModels(false);
    }
  }, [provider, apiKeys]);

  // 프로바이더가 Ollama로 변경될 때 자동으로 모델 목록 새로고침
  useEffect(() => {
    const fetchOllamaModels = async () => {
      if (provider !== AIServiceProvider.Ollama) {
        // Ollama가 아니면 동적 모델 목록 초기화
        setDynamicModels({});
        return;
      }

      const apiKey = apiKeys[provider];
      if (!apiKey) {
        logger.warn('No API key available for Ollama');
        return;
      }

      setIsRefreshingModels(true);
      try {
        const service = AIServiceFactory.getService(provider, apiKey);
        const modelList = await service.listModels();

        // ModelInfo[] 배열을 Record<string, ModelInfo>로 변환
        const modelsRecord = modelList.reduce(
          (acc, modelInfo) => {
            const key = modelInfo.id || modelInfo.name;
            acc[key] = modelInfo;
            return acc;
          },
          {} as Record<string, ModelInfo>,
        );

        setDynamicModels(modelsRecord);
        logger.info(`Auto-refreshed ${modelList.length} models from Ollama server`);
      } catch (error) {
        logger.error('Failed to auto-refresh models:', error);
        // 에러 시 빈 객체로 설정하여 정적 모델로 fallback
        setDynamicModels({});
      } finally {
        setIsRefreshingModels(false);
      }
    };

    fetchOllamaModels();
  }, [provider, apiKeys]); // 정확한 의존성만 포함

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();
    logger.info('📊 Available providers:', providers);
    
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

      if (
        newProvider === AIServiceProvider.Ollama &&
        Object.keys(dynamicModels).length > 0
      ) {
        availableModels = dynamicModels;
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
