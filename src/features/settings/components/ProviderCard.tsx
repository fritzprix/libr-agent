import React, { useState } from 'react';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Input,
} from '@/components/ui';
import { useDebounce } from '@/hooks/useDebounce';

export interface ProviderCardProps {
  provider: AIServiceProvider;
  providerName: string;
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
  apiKey,
  baseUrl,
  onPendingChange,
}: ProviderCardProps) {
  const [localApiKey, setLocalApiKey] = useState(apiKey || '');
  const [localBaseUrl, setLocalBaseUrl] = useState(baseUrl || '');

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
      </CardHeader>
      <CardContent className="space-y-3 min-w-0">
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 text-sm font-medium">
            API Key
          </label>
          <Input
            type="password"
            placeholder={`Enter your ${providerName} API key`}
            value={localApiKey}
            onChange={(e) => {
              const v = e.target.value;
              setLocalApiKey(v);
              schedulePending({ apiKey: v });
            }}
            onBlur={() => onPendingChange(provider, { apiKey: localApiKey })}
            className="bg-background border text-foreground w-full"
          />
        </div>

        {(provider === AIServiceProvider.Ollama ||
          provider === AIServiceProvider.OpenAI) && (
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 text-sm font-medium">
              Base URL
            </label>
            <Input
              type="url"
              placeholder="http://localhost:11434"
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
      </CardContent>
    </Card>
  );
}

export const ProviderCard = React.memo(ProviderCardBase, (prev, next) => {
  return (
    prev.apiKey === next.apiKey && (prev.baseUrl || '') === (next.baseUrl || '')
  );
});
