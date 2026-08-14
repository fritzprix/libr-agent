import React, { useState } from 'react';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Button,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
} from '@/components/ui';
import { Eye, EyeOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export interface ProviderCardProps {
  provider: AIServiceProvider;
  providerName: string;
  /** Optional one-liner shown below the card title */
  description?: string;
  apiKey: string;
  baseUrl?: string;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
}

function ProviderCardBase({
  provider,
  providerName,
  description,
  apiKey,
  baseUrl,
  onPendingChange,
}: ProviderCardProps) {
  const [showApiKey, setShowApiKey] = useState(false);
  const { t } = useTranslation('common');

  const baseUrlPlaceholder =
    provider === AIServiceProvider.OpenAI
      ? t(
          'settings.provider.openaiBaseUrlPlaceholder',
          'https://api.openai.com/v1 (optional proxy)',
        )
      : t('settings.provider.baseUrlPlaceholder', 'http://localhost:11434');

  return (
    <Card className="bg-background border shadow-sm min-w-0 w-full">
      <CardHeader className="pb-4">
        <CardTitle className="text-foreground text-base font-medium break-words">
          {providerName}
        </CardTitle>
        {description && (
          <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
        )}
      </CardHeader>
      <CardContent className="space-y-3 min-w-0">
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            {t('settings.provider.apiKey', 'API Key')}
          </label>
          <div className="relative">
            <Input
              type={showApiKey ? 'text' : 'password'}
              placeholder={t('settings.provider.apiKeyPlaceholder', {
                name: providerName,
                defaultValue: 'Enter your {{name}} API key',
              })}
              value={apiKey}
              // Keep provider settings controlled by the form state.
              // A debounced/local mirror caused saves and downstream model refreshes
              // to read stale URLs/keys before the latest keystroke reached the form.
              onChange={(e) => {
                onPendingChange(provider, { apiKey: e.target.value });
              }}
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

        {(provider === AIServiceProvider.Ollama ||
          provider === AIServiceProvider.OpenAI) && (
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 text-sm font-medium">
              {t('settings.provider.baseUrl', 'Base URL')}
            </label>
            <Input
              type="url"
              placeholder={baseUrlPlaceholder}
              value={baseUrl || ''}
              // Regression note: model pickers depend on the same pending form state.
              // Do not reintroduce debounced/local buffering here unless save/refresh
              // paths are explicitly flushed first.
              onChange={(e) => {
                onPendingChange(provider, { baseUrl: e.target.value });
              }}
              className="bg-background border text-foreground w-full"
            />
            {provider === AIServiceProvider.OpenAI && (
              <p className="text-xs text-muted-foreground mt-1">
                {t(
                  'settings.provider.openaiBaseUrlHint',
                  'For vLLM, LM Studio, LocalAI, and other OpenAI-compatible servers, add a Custom OpenAI Provider below.',
                )}
              </p>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

export const ProviderCard = React.memo(ProviderCardBase, (prev, next) => {
  return (
    prev.apiKey === next.apiKey &&
    (prev.baseUrl || '') === (next.baseUrl || '') &&
    prev.description === next.description &&
    prev.onPendingChange === next.onPendingChange // Critical: check callback stability
  );
});
