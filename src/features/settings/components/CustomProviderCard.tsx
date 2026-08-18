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
} from '@/components/ui';
import { Eye, EyeOff, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { normalizeManualModels } from '@/lib/ai-service/custom-providers';
import { ReasoningBudgetAdvancedSection } from './ReasoningBudgetAdvancedSection';

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
  const { t } = useTranslation('common');

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

        <ReasoningBudgetAdvancedSection
          idPrefix={provider.id}
          reasoningBudget={provider.reasoningBudget}
          reasoningBudgetMessage={provider.reasoningBudgetMessage}
          onChange={(patch) => onChange(provider.id, patch)}
        />
      </CardContent>
    </Card>
  );
}

export const CustomProviderCard = React.memo(
  CustomProviderCardBase,
  (prev, next) => {
    return (
      prev.provider.id === next.provider.id &&
      prev.provider.name === next.provider.name &&
      prev.provider.baseUrl === next.provider.baseUrl &&
      (prev.provider.apiKey || '') === (next.provider.apiKey || '') &&
      modelsToText(prev.provider.models) ===
        modelsToText(next.provider.models) &&
      prev.provider.reasoningBudget === next.provider.reasoningBudget &&
      (prev.provider.reasoningBudgetMessage || '') ===
        (next.provider.reasoningBudgetMessage || '')
    );
  },
);
