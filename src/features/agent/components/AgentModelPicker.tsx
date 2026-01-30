import { FC, useCallback, useMemo, useState, useEffect } from 'react';
import { Dropdown } from '@/components/ui';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import { getLogger } from '@/lib/logger';
import { RefreshCw } from 'lucide-react';
import { useSettings } from '@/hooks/use-settings';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import useSWR from 'swr';

const logger = getLogger('AgentModelPicker');

interface AgentModelPickerProps {
  currentModel?: string;
  currentProvider?: string;
  className?: string;
  onConfigUpdate?: (model: string, provider: string) => void;
}

export const AgentModelPicker: FC<AgentModelPickerProps> = ({
  currentModel,
  currentProvider,
  className,
  onConfigUpdate,
}) => {
  const {
    value: { serviceConfigs },
  } = useSettings();

  // Local state for the picker selections (initialized from props)
  const [localProvider, setLocalProvider] = useState<string>(
    currentProvider || '',
  );
  const [localModel, setLocalModel] = useState<string>(currentModel || '');

  // Sync state when props change (e.g. initial load or external update)
  useEffect(() => {
    if (currentProvider) setLocalProvider(currentProvider);
    if (currentModel) setLocalModel(currentModel);
  }, [currentProvider, currentModel]);

  // --- Model Fetching Logic (Mirrors ModelProvider but scoped to localProvider) ---

  // Get API key for the selected local provider
  const apiKey = useMemo(() => {
    return serviceConfigs[localProvider as AIServiceProvider]?.apiKey || '';
  }, [serviceConfigs, localProvider]);

  // Fetcher for dynamic models
  const fetchDynamicModels = useCallback(
    async ([, provider, key]: [string, string, string]) => {
      const supportsDynamic =
        provider === AIServiceProvider.Ollama ||
        provider === AIServiceProvider.OpenAI ||
        provider === AIServiceProvider.Anthropic ||
        provider === AIServiceProvider.Gemini;

      if (!supportsDynamic) return {};

      let effectiveApiKey = key;
      if (!effectiveApiKey) {
        if (provider === AIServiceProvider.Ollama) {
          effectiveApiKey = 'ollama-dummy';
        } else {
          return {};
        }
      }

      try {
        const providerConfig =
          serviceConfigs[provider as AIServiceProvider] || {};
        const service = AIServiceFactory.getService(
          provider as AIServiceProvider,
          effectiveApiKey,
          providerConfig,
        );
        const modelList = await service.listModels();

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
    localProvider ? ['local-models', localProvider, apiKey] : null,
    fetchDynamicModels,
    {
      revalidateOnFocus: false,
      dedupingInterval: 60000,
    },
  );

  // Combine static and dynamic models
  const availableModels = useMemo(() => {
    if (!localProvider) return {};

    const providerConfig =
      serviceConfigs[localProvider as AIServiceProvider] || {};

    // If 3rd party is enabled for OpenAI, show only custom model ID
    if (
      localProvider === AIServiceProvider.OpenAI &&
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
      llmConfigManager.getModelsForProvider(
        localProvider as AIServiceProvider,
      ) || {};

    return Object.keys(dynamicModels).length > 0 ? dynamicModels : staticModels;
  }, [localProvider, dynamicModels, serviceConfigs]);

  const modelOptions = useMemo(() => {
    return Object.entries(availableModels).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, [availableModels]);

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();
    return Object.entries(providers).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, []);

  // --- Update Logic ---

  // Handle Provider Change
  const handleProviderChange = useCallback(
    (newProvider: string) => {
      setLocalProvider(newProvider);
      onConfigUpdate?.(localModel, newProvider);

      // Default model selection logic (optional, can be moved to parent or kept here as UI helper)
      const staticModels = llmConfigManager.getModelsForProvider(
        newProvider as AIServiceProvider,
      );
      if (staticModels && Object.keys(staticModels).length > 0) {
        const defaultModel = Object.keys(staticModels)[0];
        setLocalModel(defaultModel);
        onConfigUpdate?.(defaultModel, newProvider);
      } else {
        setLocalModel('');
        onConfigUpdate?.('', newProvider);
      }
    },
    [localModel, onConfigUpdate],
  );

  // Handle Model Change
  const handleModelChange = useCallback(
    (newModel: string) => {
      setLocalModel(newModel);
      onConfigUpdate?.(newModel, localProvider);
    },
    [localProvider, onConfigUpdate],
  );

  if (!localProvider && !localModel) return null;

  return (
    <div
      className={`flex items-center space-x-2 bg-muted/50 border border-primary/20 rounded-lg px-2 py-1 font-mono text-xs ${className}`}
    >
      <div className="w-2 h-2 rounded-full bg-primary/40" />

      {/* Provider Selector */}
      <Dropdown
        options={providerOptions}
        value={localProvider}
        onChange={handleProviderChange}
        className="w-24 h-6 text-xs bg-transparent border-none focus:ring-0"
        placeholder="Provider"
      />

      <span className="text-muted-foreground/50">/</span>

      {/* Model Selector */}
      <Dropdown
        options={modelOptions}
        value={localModel}
        onChange={handleModelChange}
        className="min-w-32 h-6 text-xs bg-transparent border-none focus:ring-0"
        placeholder={isRefreshing ? 'Loading...' : 'Model'}
        disabled={isRefreshing || !localProvider}
      />

      {/* Refresh Button for Ollama */}
      {localProvider === AIServiceProvider.Ollama && (
        <button
          onClick={() => refreshModels()}
          disabled={isRefreshing}
          className="p-1 hover:bg-primary/10 rounded text-muted-foreground hover:text-primary transition-colors"
          title="Refresh models"
        >
          <RefreshCw
            className={`w-3 h-3 ${isRefreshing ? 'animate-spin' : ''}`}
          />
        </button>
      )}
    </div>
  );
};
