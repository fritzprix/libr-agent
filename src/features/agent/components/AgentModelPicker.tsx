import { FC, useCallback, useMemo } from 'react';
import React from 'react';
import { RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { AIServiceProvider } from '@/lib/ai-service';
import {
  isCustomOpenAIProviderId,
  listCustomProviderPickerOptions,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service/custom-providers';
import { llmConfigManager } from '@/lib/llm-config-manager';
import { cn } from '@/lib/utils';
import { useSettings } from '@/hooks/use-settings';
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
  const { t } = useTranslation('common');
  const {
    value: { customProviders, serviceConfigs },
  } = useSettings();
  const {
    availableModels,
    isRefreshing,
    refreshModels,
    canRefresh,
    refreshBlockedReason,
  } = useAgentModels(currentProvider);

  const modelOptions = useMemo(() => {
    return Object.entries(availableModels).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
  }, [availableModels]);

  const providerOptions = useMemo(() => {
    const providers = llmConfigManager.getProviders();
    const builtin = Object.entries(providers).map(([key, value]) => ({
      label: value.name,
      value: key,
    }));
    return [...builtin, ...listCustomProviderPickerOptions(customProviders)];
  }, [customProviders]);

  const showRefreshButton =
    Boolean(currentProvider) &&
    (canRefresh ||
      refreshBlockedReason === 'missing-api-key' ||
      refreshBlockedReason === 'missing-base-url');

  const refreshButtonLabel = useMemo(() => {
    if (canRefresh) {
      return t('agent.modelPicker.refreshModels');
    }

    if (refreshBlockedReason === 'missing-api-key') {
      return t('agent.modelPicker.refreshRequiresApiKey', {
        defaultValue: 'Add an API key to enable model refresh',
      });
    }

    if (refreshBlockedReason === 'missing-base-url') {
      return t('agent.modelPicker.refreshRequiresBaseUrl', {
        defaultValue: 'Add a base URL to enable model refresh',
      });
    }

    return '';
  }, [canRefresh, refreshBlockedReason, t]);

  const handleProviderChange = useCallback(
    (newProvider: string) => {
      let defaultModel = '';

      if (isCustomOpenAIProviderId(newProvider)) {
        const resolved = resolveProviderRuntimeConfig(newProvider, {
          serviceConfigs,
          customProviders,
        });
        defaultModel = resolved.manualModels?.[0] ?? '';
      } else {
        const staticModels = llmConfigManager.getModelsForProvider(
          newProvider as AIServiceProvider,
        );
        if (staticModels && Object.keys(staticModels).length > 0) {
          defaultModel = Object.keys(staticModels)[0];
        }
      }

      onConfigUpdate?.(defaultModel, newProvider);
    },
    [customProviders, onConfigUpdate, serviceConfigs],
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
      className={cn(
        'flex max-w-full items-center gap-1.5 overflow-hidden rounded-lg border border-primary/20 bg-muted/50 px-2 py-1 font-mono text-xs',
        disabled && 'pointer-events-none opacity-50',
        className,
      )}
    >
      <div className="w-2 h-2 rounded-full bg-primary/40" />

      {/* Provider Selector */}
      <Select
        value={currentProvider}
        onValueChange={handleProviderChange}
        disabled={disabled}
      >
        <SelectTrigger className="h-6 w-[5.5rem] shrink-0 border-none bg-transparent px-1 text-xs shadow-none gap-1 focus:ring-0 [&>span]:truncate">
          <SelectValue placeholder={t('agent.modelPicker.provider')} />
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
        <SelectTrigger className="h-6 min-w-0 flex-1 border-none bg-transparent px-1 text-xs shadow-none gap-1 focus:ring-0 sm:w-[10.5rem] sm:flex-none [&>span]:truncate">
          <SelectValue
            placeholder={
              isRefreshing
                ? t('agent.modelPicker.loading')
                : t('agent.modelPicker.model')
            }
          />
        </SelectTrigger>
        <SelectContent>
          {modelOptions.map((opt) => (
            <SelectItem key={opt.value} value={opt.value}>
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {showRefreshButton && (
        <Tooltip>
          <TooltipTrigger asChild>
            <span
              tabIndex={disabled || isRefreshing || !canRefresh ? 0 : undefined}
              className={cn(
                'inline-block rounded focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                (disabled || isRefreshing || !canRefresh) &&
                  'cursor-not-allowed opacity-50',
              )}
              aria-label={
                disabled || isRefreshing || !canRefresh
                  ? refreshButtonLabel
                  : undefined
              }
              aria-disabled={
                disabled || isRefreshing || !canRefresh ? true : undefined
              }
              role={
                disabled || isRefreshing || !canRefresh ? 'button' : undefined
              }
            >
              <button
                type="button"
                onClick={() => refreshModels()}
                disabled={disabled || isRefreshing || !canRefresh}
                className={cn(
                  'p-1 hover:bg-primary/10 rounded text-muted-foreground hover:text-primary transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                  (disabled || isRefreshing || !canRefresh) &&
                    'pointer-events-none',
                )}
                aria-label={
                  disabled || isRefreshing || !canRefresh
                    ? undefined
                    : refreshButtonLabel
                }
                aria-hidden={
                  disabled || isRefreshing || !canRefresh ? true : undefined
                }
              >
                <RefreshCw
                  className={`w-3 h-3 ${isRefreshing ? 'animate-spin' : ''}`}
                />
              </button>
            </span>
          </TooltipTrigger>
          <TooltipContent>{refreshButtonLabel}</TooltipContent>
        </Tooltip>
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
