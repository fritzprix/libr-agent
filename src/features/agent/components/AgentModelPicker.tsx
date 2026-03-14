import { FC, useCallback, useMemo } from 'react';
import React from 'react';
import { RefreshCw } from 'lucide-react';

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { AIServiceProvider } from '@/lib/ai-service';
import { llmConfigManager } from '@/lib/llm-config-manager';
import { useAgentModels } from '../hooks/useAgentModels';

interface AgentModelPickerProps {
  currentModel?: string;
  currentProvider?: string;
  className?: string;
  disabled?: boolean;
  onConfigUpdate?: (model: string, provider: string) => void;
}

const AgentModelPickerComponent: FC<AgentModelPickerProps> = ({
  currentModel = '',
  currentProvider = '',
  className,
  disabled = false,
  onConfigUpdate,
}) => {
  const { availableModels, isRefreshing, refreshModels } =
    useAgentModels(currentProvider);

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

  const handleProviderChange = useCallback(
    (newProvider: string) => {
      // Default model selection logic
      const staticModels = llmConfigManager.getModelsForProvider(
        newProvider as AIServiceProvider,
      );
      let defaultModel = '';
      if (staticModels && Object.keys(staticModels).length > 0) {
        defaultModel = Object.keys(staticModels)[0];
      }

      onConfigUpdate?.(defaultModel, newProvider);
    },
    [onConfigUpdate],
  );

  const handleModelChange = useCallback(
    (newModel: string) => {
      onConfigUpdate?.(newModel, currentProvider);
    },
    [currentProvider, onConfigUpdate],
  );

  if (!currentProvider && !currentModel) return null;

  return (
    <div
      className={`flex items-center space-x-2 bg-muted/50 border border-primary/20 rounded-lg px-2 py-1 font-mono text-xs ${
        disabled ? 'opacity-50 pointer-events-none' : ''
      } ${className}`}
    >
      <div className="w-2 h-2 rounded-full bg-primary/40" />

      {/* Provider Selector */}
      <Select
        value={currentProvider}
        onValueChange={handleProviderChange}
        disabled={disabled}
      >
        <SelectTrigger className="w-24 h-6 text-xs bg-transparent border-none focus:ring-0 shadow-none px-1 gap-1">
          <SelectValue placeholder="Provider" />
        </SelectTrigger>
        <SelectContent>
          {providerOptions.map((opt) => (
            <SelectItem key={opt.value} value={opt.value}>
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <span className="text-muted-foreground/50">/</span>

      {/* Model Selector */}
      <Select
        value={currentModel}
        onValueChange={handleModelChange}
        disabled={disabled || isRefreshing || !currentProvider}
      >
        <SelectTrigger className="min-w-32 h-6 text-xs bg-transparent border-none focus:ring-0 shadow-none px-1 gap-1">
          <SelectValue placeholder={isRefreshing ? 'Loading...' : 'Model'} />
        </SelectTrigger>
        <SelectContent>
          {modelOptions.map((opt) => (
            <SelectItem key={opt.value} value={opt.value}>
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {/* Refresh Button — available for all providers */}
      {currentProvider && (
        <button
          onClick={() => refreshModels()}
          disabled={disabled || isRefreshing}
          className="p-1 hover:bg-primary/10 rounded text-muted-foreground hover:text-primary transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
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

export const AgentModelPicker = React.memo(
  AgentModelPickerComponent,
  (prev, next) => {
    return (
      prev.currentModel === next.currentModel &&
      prev.currentProvider === next.currentProvider &&
      prev.className === next.className &&
      prev.disabled === next.disabled &&
      prev.onConfigUpdate === next.onConfigUpdate
    );
  },
);
