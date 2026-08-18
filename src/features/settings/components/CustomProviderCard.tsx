import React, { useState } from 'react';
import type { CustomOpenAIProvider } from '@/context/SettingsContext';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Button,
  Textarea,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  Checkbox,
} from '@/components/ui';
import { ChevronDown, ChevronUp, Eye, EyeOff, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { normalizeManualModels } from '@/lib/ai-service/custom-providers';
import {
  DEFAULT_REASONING_BUDGET_MESSAGE,
  MAX_REASONING_BUDGET_TOKENS,
  parseReasoningBudgetInput,
} from '@/lib/ai-service/openai/reasoning-budget';

export interface CustomProviderCardProps {
  provider: CustomOpenAIProvider;
  onChange: (id: string, patch: Partial<CustomOpenAIProvider>) => void;
  onRemove: (id: string) => void;
}

function modelsToText(models: string[] | undefined): string {
  return (models ?? []).join('\n');
}

function textToModels(text: string): string[] | undefined {
  const parts = text
    .split(/[\n,]/)
    .map((part) => part.trim())
    .filter((part) => part.length > 0);
  return normalizeManualModels(parts);
}

function CustomProviderCardBase({
  provider,
  onChange,
  onRemove,
}: CustomProviderCardProps) {
  const [showApiKey, setShowApiKey] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(
    () =>
      provider.reasoningBudget != null ||
      Boolean(provider.reasoningBudgetMessage) ||
      provider.sendNativeReasoningBudget === true,
  );
  const { t } = useTranslation('common');
  const hasReasoningBudget = provider.reasoningBudget != null;

  return (
    <Card className="bg-background border shadow-sm min-w-0 w-full">
      <CardHeader className="pb-4 flex flex-row items-start justify-between gap-2 space-y-0">
        <div className="min-w-0 flex-1 space-y-2">
          <CardTitle className="text-foreground text-base font-medium">
            {t('settings.customProviders.cardTitle', 'Custom OpenAI Provider')}
          </CardTitle>
          <Input
            type="text"
            placeholder={t(
              'settings.customProviders.namePlaceholder',
              'e.g., Local-LMStudio, vLLM-Server-1',
            )}
            value={provider.name}
            onChange={(e) => onChange(provider.id, { name: e.target.value })}
            className="bg-background border text-foreground w-full"
            aria-label={t('settings.customProviders.name', 'Display name')}
          />
        </div>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={() => onRemove(provider.id)}
          aria-label={t(
            'settings.customProviders.remove',
            'Remove custom provider',
          )}
        >
          <Trash2 className="h-4 w-4 text-destructive" />
        </Button>
      </CardHeader>
      <CardContent className="space-y-3 min-w-0">
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            {t('settings.provider.baseUrl', 'Base URL')}
          </label>
          <Input
            type="url"
            placeholder={t(
              'settings.customProviders.baseUrlPlaceholder',
              'http://192.168.1.100:8000/v1',
            )}
            value={provider.baseUrl}
            onChange={(e) => onChange(provider.id, { baseUrl: e.target.value })}
            className="bg-background border text-foreground w-full"
          />
        </div>

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            {t('settings.provider.apiKey', 'API Key')}{' '}
            <span className="font-normal">
              ({t('settings.customProviders.optional', 'optional')})
            </span>
          </label>
          <div className="relative">
            <Input
              type={showApiKey ? 'text' : 'password'}
              placeholder={t(
                'settings.customProviders.apiKeyPlaceholder',
                'Leave empty for local servers',
              )}
              value={provider.apiKey || ''}
              onChange={(e) =>
                onChange(provider.id, { apiKey: e.target.value })
              }
              className="bg-background border text-foreground w-full pr-10"
            />
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="absolute right-0 top-0 h-full px-3 py-2 hover:bg-transparent"
                  onClick={() => setShowApiKey((v) => !v)}
                  aria-label={
                    showApiKey
                      ? t('settings.provider.hideApiKey', 'Hide API key')
                      : t('settings.provider.showApiKey', 'Show API key')
                  }
                  aria-pressed={showApiKey}
                >
                  {showApiKey ? (
                    <EyeOff className="h-4 w-4 text-muted-foreground" />
                  ) : (
                    <Eye className="h-4 w-4 text-muted-foreground" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {showApiKey
                  ? t('settings.provider.hideApiKey', 'Hide API key')
                  : t('settings.provider.showApiKey', 'Show API key')}
              </TooltipContent>
            </Tooltip>
          </div>
        </div>

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            {t('settings.customProviders.models', 'Models')}{' '}
            <span className="font-normal">
              (
              {t(
                'settings.customProviders.modelsOptional',
                'optional manual list',
              )}
              )
            </span>
          </label>
          <Textarea
            placeholder={t(
              'settings.customProviders.modelsPlaceholder',
              'One model ID per line (or comma-separated).\nLeave empty to use /v1/models.',
            )}
            value={modelsToText(provider.models)}
            onChange={(e) =>
              onChange(provider.id, { models: textToModels(e.target.value) })
            }
            className="bg-background border text-foreground w-full min-h-[72px]"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.customProviders.modelsDescription',
              'Used when the endpoint cannot list models. Otherwise models are fetched from /v1/models after save.',
            )}
          </p>
        </div>

        <div className="min-w-0 border-t pt-3">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="w-full justify-between px-0 text-sm font-medium text-foreground hover:bg-transparent"
            onClick={() => setAdvancedOpen((open) => !open)}
            aria-expanded={advancedOpen}
          >
            {t('settings.customProviders.advanced', 'Advanced')}
            {advancedOpen ? (
              <ChevronUp className="h-4 w-4 text-muted-foreground" />
            ) : (
              <ChevronDown className="h-4 w-4 text-muted-foreground" />
            )}
          </Button>

          {advancedOpen ? (
            <div className="mt-3 space-y-3">
              <div className="min-w-0">
                <label
                  htmlFor={`reasoning-budget-${provider.id}`}
                  className="block text-muted-foreground mb-2 text-sm font-medium"
                >
                  {t(
                    'settings.customProviders.reasoningBudget',
                    'Reasoning budget (tokens)',
                  )}
                </label>
                <Input
                  id={`reasoning-budget-${provider.id}`}
                  type="number"
                  min={1}
                  max={MAX_REASONING_BUDGET_TOKENS}
                  step={1}
                  placeholder={t(
                    'settings.customProviders.reasoningBudgetPlaceholder',
                    'Unlimited',
                  )}
                  value={provider.reasoningBudget ?? ''}
                  onChange={(e) => {
                    const raw = e.target.value.trim();
                    if (raw === '') {
                      onChange(provider.id, {
                        reasoningBudget: undefined,
                        sendNativeReasoningBudget: undefined,
                      });
                      return;
                    }
                    const parsed = parseReasoningBudgetInput(raw);
                    if (parsed == null) {
                      return;
                    }
                    onChange(provider.id, {
                      reasoningBudget: parsed,
                    });
                  }}
                  className="bg-background border text-foreground w-full"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.customProviders.reasoningBudgetDescription',
                    'Client-side cap on streamed thinking (approx. chars/4; may underestimate CJK). Leave empty for unlimited. Triggers one recovery retry with a stop-thinking instruction.',
                  )}
                </p>
              </div>

              <div className="min-w-0">
                <label
                  htmlFor={`reasoning-budget-message-${provider.id}`}
                  className="block text-muted-foreground mb-2 text-sm font-medium"
                >
                  {t(
                    'settings.customProviders.reasoningBudgetMessage',
                    'Budget exceeded message',
                  )}
                </label>
                <Textarea
                  id={`reasoning-budget-message-${provider.id}`}
                  placeholder={t(
                    'settings.customProviders.reasoningBudgetMessagePlaceholder',
                    DEFAULT_REASONING_BUDGET_MESSAGE,
                  )}
                  value={provider.reasoningBudgetMessage ?? ''}
                  onChange={(e) =>
                    onChange(provider.id, {
                      reasoningBudgetMessage: e.target.value || undefined,
                    })
                  }
                  className="bg-background border text-foreground w-full min-h-[72px]"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.customProviders.reasoningBudgetMessageDescription',
                    'Injected once on the recovery retry after the client aborts thinking.',
                  )}
                </p>
              </div>

              <div className="flex items-start space-x-2">
                <Checkbox
                  id={`send-native-reasoning-budget-${provider.id}`}
                  checked={provider.sendNativeReasoningBudget === true}
                  disabled={!hasReasoningBudget}
                  onCheckedChange={(checked) =>
                    onChange(provider.id, {
                      sendNativeReasoningBudget:
                        checked === true ? true : undefined,
                    })
                  }
                  className="mt-0.5"
                />
                <div className="min-w-0 space-y-1">
                  <label
                    htmlFor={`send-native-reasoning-budget-${provider.id}`}
                    className={`text-sm font-medium ${
                      hasReasoningBudget
                        ? 'cursor-pointer text-muted-foreground'
                        : 'text-muted-foreground/70'
                    }`}
                  >
                    {t(
                      'settings.customProviders.sendNativeReasoningBudget',
                      'Send native llama.cpp reasoning_budget_tokens fields',
                    )}
                  </label>
                  <p className="text-xs text-muted-foreground">
                    {t(
                      'settings.customProviders.sendNativeReasoningBudgetDescription',
                      'Off by default. Enable only for llama.cpp-compatible servers. Unknown keys return HTTP 400 on many OpenAI-compatible hosts.',
                    )}
                  </p>
                </div>
              </div>
            </div>
          ) : null}
        </div>
      </CardContent>
    </Card>
  );
}

export const CustomProviderCard = React.memo(
  CustomProviderCardBase,
  (prev, next) => {
    // Callbacks are intentionally omitted: parents recreate them when provider
    // list identity changes, and field equality already gates useful skips.
    return (
      prev.provider.id === next.provider.id &&
      prev.provider.name === next.provider.name &&
      prev.provider.baseUrl === next.provider.baseUrl &&
      (prev.provider.apiKey || '') === (next.provider.apiKey || '') &&
      modelsToText(prev.provider.models) ===
        modelsToText(next.provider.models) &&
      prev.provider.reasoningBudget === next.provider.reasoningBudget &&
      (prev.provider.reasoningBudgetMessage || '') ===
        (next.provider.reasoningBudgetMessage || '') &&
      prev.provider.sendNativeReasoningBudget ===
        next.provider.sendNativeReasoningBudget
    );
  },
);
