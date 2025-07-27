import {
  createContext,
  useCallback,
  useMemo,
  useContext,
  FC,
  PropsWithChildren,
} from 'react';
import { AIServiceProvider } from '../lib/ai-service';
import {
  llmConfigManager,
  ModelInfo,
  ProviderInfo,
} from '../lib/llm-config-manager';
import { useSettings } from '../hooks/use-settings';

const DEFAULT_MODEL_INFO: ModelInfo = {
  contextWindow: 0,
  supportTools: false,
  supportReasoning: false,
  supportStreaming: false,
  cost: { input: 0, output: 0 },
  description: '',
  name: '',
};

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

  const providerOptions = useMemo(() => {
    return Object.entries(llmConfigManager.getProviders()).map(
      ([key, value]) => ({
        label: value.name,
        value: key,
      }),
    );
  }, []);

  const modelOptions = useMemo(() => {
    return Object.entries(
      llmConfigManager.getModelsForProvider(provider) || {},
    ).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, [provider]);

  const selectedModelData = useMemo(() => {
    return llmConfigManager.getModel(provider, model) || DEFAULT_MODEL_INFO;
  }, [provider, model]);

  const setProvider = useCallback(
    (newProvider: AIServiceProvider) => {
      const models = llmConfigManager.getModelsForProvider(newProvider);
      if (!models) {
        throw new Error(`no available models for ${newProvider}`);
      }

      const modelEntries = Object.entries(models);
      let newModel = model;
      if (modelEntries.length > 0) {
        newModel = modelEntries[0][0];
      }
      update({ preferredModel: { provider: newProvider, model: newModel } });
    },
    [update, model],
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
      models: llmConfigManager.getModelsForProvider(provider) || {},
      providers: Object.values(llmConfigManager.getProviders()),
      setProvider,
      setModel,
      isLoading,
      apiKeys,
      selectedModelData,
      providerOptions,
      modelOptions,
    }),
    [
      model,
      provider,
      setProvider,
      setModel,
      isLoading,
      apiKeys,
      selectedModelData,
      providerOptions,
      modelOptions,
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
