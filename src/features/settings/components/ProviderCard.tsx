import React, { useState } from 'react';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
  Checkbox,
  Button,
} from '@/components/ui';
import { useDebounce } from '@/hooks/useDebounce';
import { Eye, EyeOff } from 'lucide-react';
import { useTranslation } from 'react-i18next';

export interface ProviderCardProps {
  provider: AIServiceProvider;
  providerName: string;
  /** Optional one-liner shown below the card title */
  description?: string;
  apiKey: string;
  baseUrl?: string;
  use3rdParty?: boolean;
  customModelId?: string;
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
  use3rdParty,
  customModelId,
  onPendingChange,
}: ProviderCardProps) {
  const [localApiKey, setLocalApiKey] = useState(apiKey || '');
  const [localBaseUrl, setLocalBaseUrl] = useState(baseUrl || '');
  const [localUse3rdParty, setLocalUse3rdParty] = useState(
    use3rdParty || false,
  );
  const [localCustomModelId, setLocalCustomModelId] = useState(
    customModelId || '',
  );
  const [showApiKey, setShowApiKey] = useState(false);
  const { t } = useTranslation('common');

  // Use debounce hook for pending changes
  const { debounced: schedulePending } = useDebounce(
    (patch: Partial<ServiceConfig>) => {
      onPendingChange(provider, patch);
    },
    350,
  );

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
              value={localApiKey}
              onChange={(e) => {
                const v = e.target.value;
                setLocalApiKey(v);
                schedulePending({ apiKey: v });
              }}
              onBlur={() => onPendingChange(provider, { apiKey: localApiKey })}
              className="bg-background border text-foreground w-full pr-10"
            />
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
              placeholder={t(
                'settings.provider.baseUrlPlaceholder',
                'http://localhost:11434',
              )}
              value={localBaseUrl}
              onChange={(e) => {
                const v = e.target.value;
                setLocalBaseUrl(v);
                schedulePending({ baseUrl: v });
              }}
              onBlur={() =>
                onPendingChange(provider, { baseUrl: localBaseUrl })
              }
              className="bg-background border text-foreground w-full"
            />
          </div>
        )}

        {provider === AIServiceProvider.OpenAI && (
          <>
            <div className="flex items-center space-x-2 min-w-0">
              <Checkbox
                id={`use3rdParty-${provider}`}
                checked={localUse3rdParty}
                onCheckedChange={(checked) => {
                  const value = checked === true;
                  setLocalUse3rdParty(value);
                  onPendingChange(provider, { use3rdParty: value });
                }}
              />
              <label
                htmlFor={`use3rdParty-${provider}`}
                className="text-sm font-medium text-muted-foreground cursor-pointer"
              >
                {t(
                  'settings.provider.use3rdParty',
                  'Use 3rd party OpenAI-compatible API',
                )}
              </label>
            </div>

            {localUse3rdParty && (
              <div className="min-w-0">
                <label className="block text-muted-foreground mb-2 text-sm font-medium">
                  {t('settings.provider.customModelId', 'Custom Model ID')}
                </label>
                <Input
                  type="text"
                  placeholder={t(
                    'settings.provider.customModelIdPlaceholder',
                    'e.g., llama-3.1-70b, mistral-large',
                  )}
                  value={localCustomModelId}
                  onChange={(e) => {
                    const v = e.target.value;
                    setLocalCustomModelId(v);
                    schedulePending({ customModelId: v });
                  }}
                  onBlur={() =>
                    onPendingChange(provider, {
                      customModelId: localCustomModelId,
                    })
                  }
                  className="bg-background border text-foreground w-full"
                />
                <p className="text-xs text-muted-foreground mt-1">
                  {t(
                    'settings.provider.customModelIdDescription',
                    'Enter the model ID supported by your 3rd party API (e.g., LM Studio, LocalAI)',
                  )}
                </p>
              </div>
            )}
          </>
        )}
      </CardContent>
    </Card>
  );
}

export const ProviderCard = React.memo(ProviderCardBase, (prev, next) => {
  return (
    prev.apiKey === next.apiKey &&
    (prev.baseUrl || '') === (next.baseUrl || '') &&
    prev.use3rdParty === next.use3rdParty &&
    (prev.customModelId || '') === (next.customModelId || '') &&
    prev.description === next.description &&
    prev.onPendingChange === next.onPendingChange // Critical: check callback stability
  );
});
