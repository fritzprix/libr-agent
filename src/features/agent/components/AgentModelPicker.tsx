import { FC, useCallback, useMemo, useState, useEffect } from 'react';
import { Dropdown } from '@/components/ui';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import { invoke } from '@tauri-apps/api/core';
import { getLogger } from '@/lib/logger';
import { Loader2, RefreshCw } from 'lucide-react';
import { useSettings } from '@/hooks/use-settings';
import { llmConfigManager, ModelInfo } from '@/lib/llm-config-manager';
import useSWR from 'swr';
import { Assistant } from '@/models/chat';

const logger = getLogger('AgentModelPicker');

interface AgentModelPickerProps {
  sessionId: string;
  currentModel?: string;
  currentProvider?: string;
  className?: string;
  onConfigUpdate?: (model: string, provider: string) => void;
  // We need the full config to ensure preservation of other fields if the backend requires full replace
  // But our backend implementation `update_session_config` takes `AgentConfig` and replaces it.
  // The `AgentConfig` struct has all fields.
  // We should ideally fetch the latest config or use what we have.
  // For now, we'll assume we update ONLY model/provider if we can, but we likely need to pass the full object.
  // Since `onConfigUpdate` is for parent notification, internal logic handles the DB update.
  // BUT: `agent_update_session_config` takes `UpdateAgentConfigRequest` which has `agentConfig`.
  // If we only send model/provider, other fields might be lost if we don't send them.
  // The `AgentModelPicker` should ideally receive the *entire* assistant object to clone and modify.
  // The `AgentModelPicker` should ideally receive the *entire* assistant object to clone and modify.
  currentAssistantConfig?: Assistant;
}

export const AgentModelPicker: FC<AgentModelPickerProps> = ({
  sessionId,
  currentModel,
  currentProvider,
  currentAssistantConfig,
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
    const staticModels =
      llmConfigManager.getModelsForProvider(
        localProvider as AIServiceProvider,
      ) || {};
    if (Object.keys(dynamicModels).length > 0) {
      return dynamicModels;
    }
    return staticModels;
  }, [localProvider, dynamicModels]);

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

  const [isUpdating, setIsUpdating] = useState(false);

  // Handle Provider Change
  const handleProviderChange = useCallback((newProvider: string) => {
    setLocalProvider(newProvider);
    // When provider changes, select the first available model or clear localModel
    // We can't do this immediately as we might need to fetch models first.
    // Logic: just set provider, let the user pick the model, OR pick first available once models load.
    // For now, reset model to empty to force selection, or pick a default if we have static ones.
    const staticModels = llmConfigManager.getModelsForProvider(
      newProvider as AIServiceProvider,
    );
    if (staticModels && Object.keys(staticModels).length > 0) {
      setLocalModel(Object.keys(staticModels)[0]);
    } else {
      setLocalModel('');
    }
  }, []);

  // Confirm / Save Changes
  // We trigger the update when the user selects a model.
  // OR we could have a specific "Save" button if we want.
  // ModelPicker usually updates immediately on selection.
  const handleModelChange = useCallback(
    async (newModel: string) => {
      setLocalModel(newModel);

      if (!sessionId || !currentAssistantConfig) {
        logger.warn('Missing session ID or config for update');
        return;
      }

      setIsUpdating(true);
      try {
        // Construct updated config
        // Assuming currentAssistantConfig holds the rest of the config
        const updatedConfig = {
          ...currentAssistantConfig,
          provider: localProvider,
          model: newModel,
          // Ensure we handle defaults if fields are missing
          temperature: currentAssistantConfig.temperature ?? 0.7,
          // Rust AgentConfig might have different field names if not mapped 1:1 via serde?
          // "AgentConfig" in Rust vs "Assistant" in TS.
          // Check rust AgentConfig: name, description, system_prompt, model, provider, etc.
          // Check TS Assistant: name, description, systemPrompt, ...
          // We need to map camelCase (TS) to snake_case (Rust) manually if they differ?
          // Rust `AgentConfig` has `#[serde(rename_all = "camelCase")]`?
          // Let's check `src-tauri/src/agent/config.rs`.
          // Yes, standard tauri commands usually use camelCase JSON.
        };

        // If strict types are required, ensure all mandatory fields are present.
        // Rust AgentConfig: name, system_prompt, model, provider.
        if (!updatedConfig.name) updatedConfig.name = 'Assistant';
        if (!updatedConfig.systemPrompt)
          updatedConfig.systemPrompt = 'You are a helpful assistant.';

        await invoke('agent_update_session_config', {
          request: {
            sessionId,
            agentConfig: updatedConfig,
          },
        });

        onConfigUpdate?.(newModel, localProvider);
        logger.info(
          `Updated session ${sessionId} to ${localProvider}/${newModel}`,
        );
      } catch (err) {
        logger.error('Failed to update agent config', err);
        // Revert UI on error?
        setLocalModel(currentModel || '');
        setLocalProvider(currentProvider || '');
      } finally {
        setIsUpdating(false);
      }
    },
    [
      sessionId,
      currentAssistantConfig,
      localProvider,
      currentModel,
      currentProvider,
      onConfigUpdate,
    ],
  );

  if (!localProvider && !localModel) return null;

  return (
    <div
      className={`flex items-center space-x-2 bg-muted/50 border border-primary/20 rounded-lg px-2 py-1 font-mono text-xs ${className}`}
    >
      {isUpdating ? (
        <Loader2 className="w-3 h-3 animate-spin text-primary" />
      ) : (
        <div className="w-2 h-2 rounded-full bg-primary/40" />
      )}

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
        className="min-w-[120px] h-6 text-xs bg-transparent border-none focus:ring-0"
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
