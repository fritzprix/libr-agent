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
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { setLastSelectedModel } from '@/lib/ai-service/last-selected-model-storage';
import {
  decodeModelChoice,
  encodeModelChoice,
} from '@/lib/ai-service/model-choice-encoding';
import { mapThinkingEffort } from '@/lib/ai-service/thinking-effort-mapping';
import type { ThinkingEffort } from '@/lib/ai-service/thinking-effort-mapping';
import { AIServiceProvider } from '@/lib/ai-service/types';
import type {
  CustomOpenAIProvider,
  ServiceConfig,
} from '@/lib/services/settings-service';
import { cn } from '@/lib/utils';
import { useSettings } from '@/hooks/use-settings';
import { ThinkingEffortControl } from '@/features/settings/components/ThinkingEffortControl';
import { useGroupedAgentModels } from '../hooks/useGroupedAgentModels';

interface AgentModelPickerProps {
  currentModel?: string;
  currentProvider?: string;
  className?: string;
  disabled?: boolean;
  onConfigUpdate?: (model: string, provider: string) => void;
  thinkingEffort?: ThinkingEffort;
  onThinkingEffortChange?: (effort: ThinkingEffort) => void;
  showThinkingEffort?: boolean;
  /**
   * Optional override for custom providers (e.g. settings form draft).
   * Falls back to persisted settings when omitted.
   */
  customProviders?: CustomOpenAIProvider[];
  /**
   * Optional override for service configs (e.g. settings form draft).
   */
  serviceConfigs?: Record<string, ServiceConfig>;
}

const AgentModelPickerComponent: FC<AgentModelPickerProps> = ({
  currentModel = '',
  currentProvider = '',
  className,
  disabled = false,
  onConfigUpdate,
  thinkingEffort,
  onThinkingEffortChange,
  showThinkingEffort = false,
  customProviders: customProvidersProp,
  serviceConfigs: serviceConfigsProp,
}) => {
  const { t } = useTranslation('common');
  const {
    value: { advanced },
  } = useSettings();

  const effectiveThinkingEffort = thinkingEffort ?? advanced.thinkingEffort;

  const {
    groupedModels,
    hasConfiguredProviders,
    isRefreshing,
    refreshModels,
    canRefresh,
    refreshBlockedReason,
    getModelInfo,
  } = useGroupedAgentModels({
    customProviders: customProvidersProp,
    serviceConfigs: serviceConfigsProp,
    currentProvider,
  });

  const selectedModelChoice = useMemo(() => {
    if (!currentProvider || !currentModel) {
      return undefined;
    }
    return encodeModelChoice(currentProvider, currentModel);
  }, [currentModel, currentProvider]);

  const selectedModelInfo = useMemo(() => {
    if (!currentProvider || !currentModel) {
      return undefined;
    }
    return getModelInfo(currentProvider, currentModel);
  }, [currentModel, currentProvider, getModelInfo]);

  const selectedProviderLabel = useMemo(() => {
    const group = groupedModels.find(
      (entry) => entry.providerId === currentProvider,
    );
    return group?.label ?? currentProvider;
  }, [currentProvider, groupedModels]);

  const triggerLabel = useMemo(() => {
    if (!currentProvider || !currentModel) {
      return undefined;
    }

    const modelName = selectedModelInfo?.name ?? currentModel;
    return `${selectedProviderLabel} · ${modelName}`;
  }, [
    currentModel,
    currentProvider,
    selectedModelInfo?.name,
    selectedProviderLabel,
  ]);

  const thinkingSupported = useMemo(() => {
    if (!currentProvider) {
      return false;
    }

    if (selectedModelInfo) {
      return selectedModelInfo.supportReasoning;
    }

    const mapped = mapThinkingEffort(
      currentProvider as AIServiceProvider,
      effectiveThinkingEffort,
    );
    return mapped.enabled;
  }, [currentProvider, effectiveThinkingEffort, selectedModelInfo]);

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

  const handleModelChoiceChange = useCallback(
    (value: string) => {
      if (!value) {
        return;
      }

      const decoded = decodeModelChoice(value);
      if (!decoded) {
        return;
      }

      setLastSelectedModel(decoded.provider, decoded.model);
      onConfigUpdate?.(decoded.model, decoded.provider);
    },
    [onConfigUpdate],
  );

  if (!hasConfiguredProviders && !currentProvider && !currentModel) {
    return null;
  }

  return (
    <div
      className={cn(
        'flex max-w-full items-center gap-1.5 overflow-hidden rounded-lg border border-primary/20 bg-muted/50 px-2 py-1 font-mono text-xs',
        disabled && 'pointer-events-none opacity-50',
        className,
      )}
    >
      <div className="h-2 w-2 shrink-0 rounded-full bg-primary/40" />

      <Select
        value={selectedModelChoice}
        onValueChange={handleModelChoiceChange}
        disabled={disabled || !hasConfiguredProviders}
      >
        <SelectTrigger className="h-6 min-w-0 flex-1 border-none bg-transparent px-1 text-xs shadow-none gap-1 focus:ring-0 sm:w-[11.5rem] sm:flex-none [&>span]:truncate">
          {triggerLabel ? (
            <span className="truncate">{triggerLabel}</span>
          ) : (
            <SelectValue
              placeholder={
                hasConfiguredProviders
                  ? isRefreshing
                    ? t('agent.modelPicker.loading')
                    : t('agent.modelPicker.model')
                  : t('agent.modelPicker.noProvidersConfigured', {
                      defaultValue: 'Configure a provider in Settings',
                    })
              }
            />
          )}
        </SelectTrigger>
        <SelectContent className="max-h-[300px]">
          {groupedModels.map((group) => {
            const modelEntries = Object.entries(group.models);
            if (modelEntries.length === 0) {
              return null;
            }

            return (
              <SelectGroup key={group.providerId}>
                <SelectLabel>{group.label}</SelectLabel>
                {modelEntries.map(([modelId, modelInfo]) => (
                  <SelectItem
                    key={encodeModelChoice(group.providerId, modelId)}
                    value={encodeModelChoice(group.providerId, modelId)}
                  >
                    {modelInfo.name}
                  </SelectItem>
                ))}
              </SelectGroup>
            );
          })}
        </SelectContent>
      </Select>

      {showThinkingEffort && onThinkingEffortChange ? (
        <>
          <span className="text-muted-foreground/50">·</span>
          <Tooltip>
            <TooltipTrigger asChild>
              <span className={cn(!thinkingSupported && 'cursor-not-allowed')}>
                <ThinkingEffortControl
                  thinkingEffort={effectiveThinkingEffort}
                  onThinkingEffortChange={onThinkingEffortChange}
                  disabled={disabled || !thinkingSupported}
                  compact
                  showTooltip={false}
                />
              </span>
            </TooltipTrigger>
            {!thinkingSupported ? (
              <TooltipContent>
                {t('agent.modelPicker.thinkingUnsupported', {
                  defaultValue: 'Thinking is not supported for this model',
                })}
              </TooltipContent>
            ) : null}
          </Tooltip>
        </>
      ) : null}

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
                  'rounded p-1 text-muted-foreground transition-colors hover:bg-primary/10 hover:text-primary focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
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
                  className={`h-3 w-3 ${isRefreshing ? 'animate-spin' : ''}`}
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
      prev.onConfigUpdate === next.onConfigUpdate &&
      prev.thinkingEffort === next.thinkingEffort &&
      prev.onThinkingEffortChange === next.onThinkingEffortChange &&
      prev.showThinkingEffort === next.showThinkingEffort &&
      prev.customProviders === next.customProviders &&
      prev.serviceConfigs === next.serviceConfigs
    );
  },
);
