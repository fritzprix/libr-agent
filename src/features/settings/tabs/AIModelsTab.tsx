import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Info } from 'lucide-react';
import { AIServiceProvider } from '@/lib/ai-service';
import {
  createCustomOpenAIProvider,
  normalizeCustomOpenAIProvider,
  toCustomProviderId,
} from '@/lib/ai-service/custom-providers';
import type {
  ServiceConfig,
  CustomOpenAIProvider,
  ModelChoice,
} from '@/lib/services/settings-service';
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import {
  Button,
  Checkbox,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { ProviderCard } from '../components/ProviderCard';
import { CustomProviderCard } from '../components/CustomProviderCard';
import { NumberSettingField } from '../components/NumberSettingField';
import { parseFloatInput } from '../components/settings-number-utils';

const EMPTY_CUSTOM_PROVIDERS: CustomOpenAIProvider[] = [];

interface AIModelsTabProps {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  customProviders: CustomOpenAIProvider[];
  providerEntries: AIServiceProvider[];
  localPreferredModel: ModelChoice;
  localFallbackModel?: ModelChoice | null;
  temperatureOverrideEnabled: boolean;
  temperature: number;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
  onCustomProvidersChange: (providers: CustomOpenAIProvider[]) => void;
  onPreferredModelChange: (model: string, provider: string) => void;
  onFallbackModelChange: (model: string, provider: string) => void;
  onTemperatureOverrideEnabledChange: (enabled: boolean) => void;
  onTemperatureChange: (temperature: number) => void;
}

function AIModelsTabComponent({
  serviceConfigs,
  customProviders,
  providerEntries,
  localPreferredModel,
  localFallbackModel,
  temperatureOverrideEnabled,
  temperature,
  onPendingChange,
  onCustomProvidersChange,
  onPreferredModelChange,
  onFallbackModelChange,
  onTemperatureOverrideEnabledChange,
  onTemperatureChange,
}: AIModelsTabProps) {
  const { t } = useTranslation('common');
  const providers = customProviders ?? EMPTY_CUSTOM_PROVIDERS;
  const PROVIDER_META: Record<
    AIServiceProvider,
    { name: string; description: string }
  > = {
    [AIServiceProvider.OpenAI]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.OpenAI}.name`,
        'OpenAI',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.OpenAI}.description`,
        'GPT-4o, o3, o4-mini and more',
      ),
    },
    [AIServiceProvider.Anthropic]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Anthropic}.name`,
        'Anthropic',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Anthropic}.description`,
        'Claude 3.5 Sonnet, Haiku and more',
      ),
    },
    [AIServiceProvider.Gemini]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Gemini}.name`,
        'Google Gemini',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Gemini}.description`,
        'Gemini 2.5 Pro, Flash and more',
      ),
    },
    [AIServiceProvider.Ollama]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Ollama}.name`,
        'Ollama',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Ollama}.description`,
        'Run open models locally on your machine',
      ),
    },
    [AIServiceProvider.Groq]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Groq}.name`,
        'Groq',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Groq}.description`,
        'Ultra-fast inference via Groq LPU chips',
      ),
    },
    [AIServiceProvider.Fireworks]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Fireworks}.name`,
        'Fireworks AI',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Fireworks}.description`,
        'Fast hosting for open-source models',
      ),
    },
    [AIServiceProvider.Cerebras]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Cerebras}.name`,
        'Cerebras',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Cerebras}.description`,
        "World's fastest AI inference chips",
      ),
    },
    [AIServiceProvider.OpenRouter]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.OpenRouter}.name`,
        'OpenRouter',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.OpenRouter}.description`,
        'Access 200+ models through one API key',
      ),
    },
    [AIServiceProvider.Empty]: {
      name: t(
        `settings.aiModels.providers.${AIServiceProvider.Empty}.name`,
        'None',
      ),
      description: t(
        `settings.aiModels.providers.${AIServiceProvider.Empty}.description`,
        '',
      ),
    },
  };

  const handleAddCustomProvider = useCallback(() => {
    const next = createCustomOpenAIProvider({
      name: '',
      baseUrl: '',
    });
    onCustomProvidersChange([...providers, next]);
  }, [providers, onCustomProvidersChange]);

  const handleCustomProviderChange = useCallback(
    (id: string, patch: Partial<CustomOpenAIProvider>) => {
      onCustomProvidersChange(
        providers.map((entry) =>
          entry.id === id
            ? normalizeCustomOpenAIProvider({ ...entry, ...patch })
            : entry,
        ),
      );
    },
    [providers, onCustomProvidersChange],
  );

  const handleRemoveCustomProvider = useCallback(
    (id: string) => {
      const providerId = toCustomProviderId(id);
      const confirmed = window.confirm(
        t(
          'settings.customProviders.removeConfirm',
          'Remove this custom provider? Sessions using it will need a new model selection.',
        ),
      );
      if (!confirmed) {
        return;
      }

      onCustomProvidersChange(providers.filter((entry) => entry.id !== id));

      if (localPreferredModel.provider === providerId) {
        onPreferredModelChange('', AIServiceProvider.OpenAI);
      }
      if (localFallbackModel?.provider === providerId) {
        onFallbackModelChange('', AIServiceProvider.OpenAI);
      }
    },
    [
      providers,
      localFallbackModel?.provider,
      localPreferredModel.provider,
      onCustomProvidersChange,
      onFallbackModelChange,
      onPreferredModelChange,
      t,
    ],
  );

  return (
    <div className="space-y-8">
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
            customProviders={providers}
            onConfigUpdate={onPreferredModelChange}
            className="w-full max-w-sm"
          />
        </div>

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.aiModels.fallbackModel', 'Fallback LLM')}
          </label>
          <AgentModelPicker
            currentModel={localFallbackModel?.model ?? ''}
            currentProvider={
              localFallbackModel?.provider ?? localPreferredModel.provider
            }
            customProviders={providers}
            onConfigUpdate={onFallbackModelChange}
            className="w-full max-w-sm"
          />
          <p className="mt-1 text-xs text-muted-foreground">
            {t(
              'settings.aiModels.fallbackModelDescription',
              'Used as a last resort when the primary model returns malformed or empty responses after all retries.',
            )}
          </p>
        </div>

        <div className="min-w-0 space-y-3">
          <div className="flex items-center space-x-2">
            <Checkbox
              id="temperature-override-enabled"
              checked={temperatureOverrideEnabled}
              onCheckedChange={(checked) => {
                onTemperatureOverrideEnabledChange(checked === true);
              }}
            />
            <label
              htmlFor="temperature-override-enabled"
              className="cursor-pointer text-sm font-medium text-muted-foreground"
            >
              {t(
                'settings.aiModels.temperatureOverride',
                'Override temperature',
              )}
            </label>
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  className="inline-flex text-muted-foreground hover:text-foreground"
                  aria-label={t(
                    'settings.aiModels.temperatureOverrideTooltipAria',
                    'About temperature defaults',
                  )}
                >
                  <Info className="h-3.5 w-3.5" />
                </button>
              </TooltipTrigger>
              <TooltipContent className="max-w-xs text-pretty">
                {t(
                  'settings.aiModels.temperatureOverrideTooltip',
                  'Most providers already set a default temperature per model. Leave this off unless you need to override that model default.',
                )}
              </TooltipContent>
            </Tooltip>
          </div>
          <p className="text-xs text-muted-foreground">
            {t(
              'settings.aiModels.temperatureOverrideDescription',
              'When disabled, provider and serving-engine defaults apply. Enable to send a custom temperature on AI requests.',
            )}
          </p>
          {temperatureOverrideEnabled ? (
            <NumberSettingField
              label={t('settings.aiModels.temperature', 'Temperature')}
              description={t(
                'settings.aiModels.temperatureDescription',
                'Controls randomness. Lower is more deterministic; higher is more creative. Range 0–2.',
              )}
              value={temperature}
              min={0}
              max={2}
              step={0.1}
              allowDecimal
              placeholder="0.7"
              parseValue={(rawValue) =>
                parseFloatInput(rawValue, {
                  fallback: temperature,
                  min: 0,
                  max: 2,
                })
              }
              onValueChange={onTemperatureChange}
              containerClassName="min-w-0"
            />
          ) : null}
        </div>
      </div>

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
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h3 className="text-lg font-medium text-foreground">
              {t('settings.customProviders.title', 'Custom OpenAI Providers')}
            </h3>
            <p className="text-sm text-muted-foreground mt-1">
              {t(
                'settings.customProviders.description',
                'Register multiple OpenAI-compatible endpoints (vLLM, LM Studio, LocalAI, etc.) and select them in the model picker.',
              )}
            </p>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={handleAddCustomProvider}
            className="gap-1.5"
          >
            <Plus className="h-4 w-4" />
            {t('settings.customProviders.add', 'Add Custom OpenAI Provider')}
          </Button>
        </div>

        {providers.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            {t(
              'settings.customProviders.empty',
              'No custom providers yet. Add one to connect an extra OpenAI-compatible server.',
            )}
          </p>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {providers.map((provider) => (
              <CustomProviderCard
                key={provider.id}
                provider={provider}
                onChange={handleCustomProviderChange}
                onRemove={handleRemoveCustomProvider}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default React.memo(AIModelsTabComponent);
