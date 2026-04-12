import React from 'react';
import { useTranslation } from 'react-i18next';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import { Button, Slider } from '@/components/ui';
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import { ProviderCard } from '../components/ProviderCard';

const OUTPUT_TOKEN_PRESETS = [
  1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072,
] as const;
const RETRY_DELAY_PRESETS = [1000, 3000, 5000, 10000] as const;

function findNearestPresetIndex(value: number): number {
  return OUTPUT_TOKEN_PRESETS.reduce((bestIndex, preset, index) => {
    const bestDistance = Math.abs(OUTPUT_TOKEN_PRESETS[bestIndex] - value);
    const nextDistance = Math.abs(preset - value);
    return nextDistance < bestDistance ? index : bestIndex;
  }, 0);
}

function formatTokenPreset(value: number): string {
  return value >= 1024 ? `${Math.round(value / 1024)}K` : `${value}`;
}

interface AIModelsTabProps {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  providerEntries: AIServiceProvider[];
  localPreferredModel: { provider: AIServiceProvider; model: string };
  localFallbackModel?: { provider: AIServiceProvider; model: string } | null;
  localMaxRetries: number;
  localRetryDelay: number;
  localDefaultMaxOutputTokens: number;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
  onPreferredModelChange: (model: string, provider: string) => void;
  onFallbackModelChange: (model: string, provider: string) => void;
  onMaxRetriesChange: (value: number) => void;
  onRetryDelayChange: (value: number) => void;
  onDefaultMaxOutputTokensChange: (value: number) => void;
}

function AIModelsTabComponent({
  serviceConfigs,
  providerEntries,
  localPreferredModel,
  localFallbackModel,
  localMaxRetries,
  localRetryDelay,
  localDefaultMaxOutputTokens,
  onPendingChange,
  onPreferredModelChange,
  onFallbackModelChange,
  onMaxRetriesChange,
  onRetryDelayChange,
  onDefaultMaxOutputTokensChange,
}: AIModelsTabProps) {
  const { t } = useTranslation('common');
  const selectedOutputTokenIndex = findNearestPresetIndex(
    localDefaultMaxOutputTokens,
  );

  // Static metadata for each provider — user-friendly name + short description
  const PROVIDER_META: Record<
    AIServiceProvider,
    { name: string; description: string }
  > = {
    [AIServiceProvider.OpenAI]: {
      name: 'OpenAI',
      description: 'GPT-4o, o3, o4-mini and more',
    },
    [AIServiceProvider.Anthropic]: {
      name: 'Anthropic',
      description: 'Claude 3.5 Sonnet, Haiku and more',
    },
    [AIServiceProvider.Gemini]: {
      name: 'Google Gemini',
      description: 'Gemini 2.5 Pro, Flash and more',
    },
    [AIServiceProvider.Ollama]: {
      name: 'Ollama',
      description: 'Run open models locally on your machine',
    },
    [AIServiceProvider.Groq]: {
      name: 'Groq',
      description: 'Ultra-fast inference via Groq LPU chips',
    },
    [AIServiceProvider.Fireworks]: {
      name: 'Fireworks AI',
      description: 'Fast hosting for open-source models',
    },
    [AIServiceProvider.Cerebras]: {
      name: 'Cerebras',
      description: "World's fastest AI inference chips",
    },
    [AIServiceProvider.OpenRouter]: {
      name: 'OpenRouter',
      description: 'Access 200+ models through one API key',
    },
    [AIServiceProvider.Empty]: { name: 'None', description: '' },
  };

  return (
    <div className="space-y-8">
      {/* Model Preference Section */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.preferences', 'Model Preferences')}
        </h3>
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.llmPreference', 'Default LLM')}
          </label>
          <AgentModelPicker
            currentModel={localPreferredModel.model}
            currentProvider={localPreferredModel.provider}
            onConfigUpdate={onPreferredModelChange}
            className="w-full max-w-sm"
          />
        </div>

        {/* Fallback Model — used when primary model fails all retries */}
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.aiModels.fallbackModel', 'Fallback LLM')}
          </label>
          <AgentModelPicker
            currentModel={localFallbackModel?.model ?? ''}
            currentProvider={
              localFallbackModel?.provider ?? localPreferredModel.provider
            }
            onConfigUpdate={onFallbackModelChange}
            className="w-full max-w-sm"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.aiModels.fallbackModelDescription',
              'Used as a last resort when the primary model returns malformed or empty responses after all retries.',
            )}
          </p>
        </div>
      </div>

      {/* API Keys Section */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.apiKeys', 'Provider API Keys')}
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {providerEntries.map((provider) => {
            const cfg = serviceConfigs[provider] || {};
            const meta = PROVIDER_META[provider];
            return (
              <ProviderCard
                key={provider}
                provider={provider}
                providerName={meta?.name ?? provider}
                description={meta?.description}
                apiKey={cfg.apiKey || ''}
                baseUrl={cfg.baseUrl}
                use3rdParty={cfg.use3rdParty}
                customModelId={cfg.customModelId}
                onPendingChange={onPendingChange}
              />
            );
          })}
        </div>
      </div>

      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.responseBehavior', 'Response Behavior')}
        </h3>

        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          <div className="min-w-0 rounded-xl border border-border/70 p-4">
            <div className="mb-4 flex items-center justify-between gap-3">
              <label className="block text-muted-foreground font-medium">
                {t(
                  'settings.advanced.maxOutputTokens',
                  'Max Output Tokens (Default)',
                )}
              </label>
              <span className="rounded-md bg-primary/10 px-2 py-1 text-sm font-mono text-primary">
                {formatTokenPreset(localDefaultMaxOutputTokens)}
              </span>
            </div>
            <Slider
              min={0}
              max={OUTPUT_TOKEN_PRESETS.length - 1}
              step={1}
              value={[selectedOutputTokenIndex]}
              onValueChange={([index]) =>
                onDefaultMaxOutputTokensChange(
                  OUTPUT_TOKEN_PRESETS[index] ?? 8192,
                )
              }
              className="w-full"
            />
            <div className="mt-3 flex flex-wrap gap-2">
              {OUTPUT_TOKEN_PRESETS.map((preset) => (
                <Button
                  key={preset}
                  type="button"
                  variant={
                    preset === localDefaultMaxOutputTokens
                      ? 'default'
                      : 'outline'
                  }
                  className="h-8 px-2 text-xs"
                  onClick={() => onDefaultMaxOutputTokensChange(preset)}
                >
                  {formatTokenPreset(preset)}
                </Button>
              ))}
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t(
                'settings.advanced.maxOutputTokensDescription',
                'Default maximum output tokens for new sessions if not specified by assistant.',
              )}
            </p>
          </div>

          <div className="min-w-0 rounded-xl border border-border/70 p-4">
            <label className="mb-2 block text-muted-foreground font-medium">
              {t('settings.advanced.maxRetries', 'Max Retry Attempts')}
            </label>
            <div className="flex max-w-xs items-center gap-2">
              <Button
                type="button"
                variant="outline"
                className="h-9 w-9 px-0"
                onClick={() =>
                  onMaxRetriesChange(Math.max(0, localMaxRetries - 1))
                }
                disabled={localMaxRetries <= 0}
                aria-label={t(
                  'settings.aiModels.decreaseRetries',
                  'Decrease retry attempts',
                )}
              >
                -
              </Button>
              <div className="flex h-9 min-w-[4rem] items-center justify-center rounded-md border bg-background px-3 text-sm font-medium">
                {localMaxRetries}
              </div>
              <Button
                type="button"
                variant="outline"
                className="h-9 w-9 px-0"
                onClick={() =>
                  onMaxRetriesChange(Math.min(5, localMaxRetries + 1))
                }
                disabled={localMaxRetries >= 5}
                aria-label={t(
                  'settings.aiModels.increaseRetries',
                  'Increase retry attempts',
                )}
              >
                +
              </Button>
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t(
                'settings.advanced.maxRetriesDescription',
                'Maximum number of retries for failed AI requests.',
              )}
            </p>

            <label className="mt-6 mb-2 block text-muted-foreground font-medium">
              {t('settings.advanced.retryDelay', 'Retry Delay (ms)')}
            </label>
            <div className="flex flex-wrap gap-2">
              {RETRY_DELAY_PRESETS.map((preset) => (
                <Button
                  key={preset}
                  type="button"
                  variant={preset === localRetryDelay ? 'default' : 'outline'}
                  className="h-8 px-3 text-xs"
                  onClick={() => onRetryDelayChange(preset)}
                >
                  {`${preset / 1000}s`}
                </Button>
              ))}
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t(
                'settings.advanced.retryDelayDescription',
                'Delay in milliseconds between retry attempts.',
              )}
            </p>
          </div>
        </div>
      </div>

      {/* Agent Hub Section (Currently Not Supported) */}
      {/* 
      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.agentHub', 'Agent Hub')}
        </h3>
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.aiModels.agentHubUrlLabel', 'Agent Hub URL')}
          </label>
          <Input
            type="url"
            placeholder="https://api.agenthub.com"
            value={localAgentHubUrl}
            onChange={(e) => onAgentHubUrlChange(e.target.value)}
            className="bg-background border text-foreground w-full"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.aiModels.agentHubUrlDescription',
              'URL of the remote Agent Hub server. If set, assistants will be synced with this server.',
            )}
          </p>
        </div>
      </div>
      */}
    </div>
  );
}

export default React.memo(AIModelsTabComponent, (prev, next) => {
  // Regression guard:
  // Provider cards are controlled by serviceConfigs. If this comparator skips
  // serviceConfigs changes, provider URL/key inputs become effectively read-only
  // because the child cards never receive the updated value prop.
  return (
    prev.serviceConfigs === next.serviceConfigs &&
    prev.providerEntries === next.providerEntries &&
    prev.localPreferredModel.provider === next.localPreferredModel.provider &&
    prev.localPreferredModel.model === next.localPreferredModel.model &&
    prev.localFallbackModel?.provider === next.localFallbackModel?.provider &&
    prev.localFallbackModel?.model === next.localFallbackModel?.model &&
    prev.localMaxRetries === next.localMaxRetries &&
    prev.localRetryDelay === next.localRetryDelay &&
    prev.localDefaultMaxOutputTokens === next.localDefaultMaxOutputTokens &&
    prev.onPendingChange === next.onPendingChange &&
    prev.onPreferredModelChange === next.onPreferredModelChange &&
    prev.onFallbackModelChange === next.onFallbackModelChange &&
    prev.onMaxRetriesChange === next.onMaxRetriesChange &&
    prev.onRetryDelayChange === next.onRetryDelayChange &&
    prev.onDefaultMaxOutputTokensChange === next.onDefaultMaxOutputTokensChange
  );
});
