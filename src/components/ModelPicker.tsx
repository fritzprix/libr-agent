import { FC, useCallback, useMemo } from "react";
import { useModelOptions } from "../context/ModelProvider";
import { AIServiceProvider } from "../lib/ai-service";
import { Dropdown } from "./ui";

interface ModelPickerProps {
  className?: string;
}

const CompactModelPicker: FC<ModelPickerProps> = ({ className = "" }) => {
  const {
    modelId,
    provider,
    setProvider,
    setModel,
    isLoading,
    apiKeys,
    selectedModelData,
    providerOptions,
    modelOptions,
  } = useModelOptions();

  const apiKeyStatus = useMemo(() => {
    const key = apiKeys[provider];
    return {
      text: provider,
      configured: key && key.length > 0,
    };
  }, [provider, apiKeys]);

  const onProviderChange = useCallback(
    (newProvider: string) => {
      setProvider(newProvider as AIServiceProvider);
    },
    [setProvider],
  );

  const onModelChange = useCallback(
    (newModel: string) => {
      setModel(newModel);
    },
    [setModel],
  );

  if (isLoading) {
    return (
      <div
        className={`font-mono text-sm text-gray-400 animate-pulse ${className}`}
      >
        [loading...]
      </div>
    );
  }

  return (
    <div
      className={`flex items-center space-x-2 bg-gray-900/70 border border-green-600/30 rounded-lg px-3 py-1 font-mono text-green-300 w-full max-w-lg mx-auto ${className}`}
    >
      {apiKeyStatus && (
        <div
          title={apiKeyStatus.text}
          className={`w-2 h-2 rounded-full flex-shrink-0 ${apiKeyStatus.configured ? "bg-green-500" : "bg-yellow-500"}`}
        ></div>
      )}
      <Dropdown
        options={providerOptions}
        value={provider}
        placeholder="provider"
        onChange={onProviderChange}
        className="flex-shrink w-28"
      />
      <span className="text-gray-600">/</span>
      <Dropdown
        options={modelOptions}
        value={modelId}
        placeholder="model"
        onChange={onModelChange}
        disabled={!modelId || modelOptions.length === 0}
        className="flex-grow min-w-0"
      />
      {selectedModelData && (
        <span className="text-xs text-gray-400 bg-gray-800 px-2 py-0.5 rounded">
          {selectedModelData.contextWindow / 1000}k
        </span>
      )}
    </div>
  );
};

// --- TERMINAL MODEL PICKER (refactored to match CompactModelPicker logic) ---
const TerminalModelPicker: FC<ModelPickerProps> = ({ className = "" }) => {
  const {
    modelId,
    provider,
    setProvider,
    setModel,
    isLoading,
    apiKeys,
    selectedModelData,
    providerOptions,
    modelOptions,
  } = useModelOptions();

  const apiKeyStatus = useMemo(() => {
    const key = apiKeys[provider];
    return {
      text: provider,
      configured: key && key.length > 0,
    };
  }, [provider, apiKeys]);

  const onProviderChange = useCallback(
    (newProvider: string) => {
      setProvider(newProvider as AIServiceProvider);
    },
    [setProvider],
  );

  const onModelChange = useCallback(
    (newModel: string) => {
      setModel(newModel);
    },
    [setModel],
  );

  if (isLoading) {
    return (
      <div
        className={`bg-gray-900 border border-green-600/30 rounded-lg p-4 font-mono text-green-300 w-full max-w-lg mx-auto flex items-center space-x-3 ${className}`}
      >
        <div className="animate-spin w-5 h-5 border-2 border-green-400 border-t-transparent rounded-full"></div>
        <span className="text-sm text-gray-400">
          Initializing LLM interface...
        </span>
      </div>
    );
  }

  return (
    <div
      className={`bg-gray-900/70 backdrop-blur-sm border border-green-600/30 rounded-lg p-4 font-mono text-green-300 w-full max-w-lg mx-auto ${className}`}
    >
      <div className="space-y-4">
        <div className="grid grid-cols-[90px_1fr_auto] gap-3 items-center">
          <label className="text-sm text-green-500">PROVIDER:</label>
          <Dropdown
            options={providerOptions}
            value={provider}
            placeholder="<select>"
            onChange={onProviderChange}
            className="w-28"
          />
          {apiKeyStatus && (
            <div
              className={`text-xs px-2 py-1 rounded font-bold ${apiKeyStatus.configured ? "bg-green-500/20 text-green-400" : "bg-yellow-500/20 text-yellow-400"}`}
            >
              {apiKeyStatus.text}
            </div>
          )}
        </div>
        <div className="grid grid-cols-[90px_1fr] gap-3 items-center">
          <label className="text-sm text-green-500">MODEL:</label>
          <Dropdown
            options={modelOptions}
            value={modelId}
            placeholder={provider ? "<select>" : "..."}
            onChange={onModelChange}
            disabled={!provider || modelOptions.length === 0}
            className="min-w-0"
          />
        </div>
        {selectedModelData && (
          <div className="border-t border-green-600/20 mt-4 pt-3 text-xs text-gray-400 space-y-2">
            <div className="flex justify-between items-center">
              <span>
                CONTEXT:{" "}
                <span className="font-semibold text-green-400">
                  {selectedModelData.contextWindow?.toLocaleString() || "N/A"}
                </span>
              </span>
              <span>
                TOOLS:{" "}
                {selectedModelData.supportTools ? (
                  <span className="font-semibold text-green-400">YES</span>
                ) : (
                  <span className="text-yellow-400">NO</span>
                )}
              </span>
              <span>
                REASONING:{" "}
                {selectedModelData.supportReasoning ? (
                  <span className="font-semibold text-green-400">YES</span>
                ) : (
                  <span className="text-yellow-400">NO</span>
                )}
              </span>
            </div>
            <div className="flex justify-between items-center text-gray-500">
              <span>
                COST (IN):{" "}
                <span className="font-semibold text-gray-400">
                  ${(selectedModelData.cost?.input * 1000)?.toFixed(2) || "?"}
                </span>
                /Mtok
              </span>
              <span>
                COST (OUT):{" "}
                <span className="font-semibold text-gray-400">
                  ${(selectedModelData.cost?.output * 1000)?.toFixed(2) || "?"}
                </span>
                /Mtok
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export { CompactModelPicker, TerminalModelPicker };
