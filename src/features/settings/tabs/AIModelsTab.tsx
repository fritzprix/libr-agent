import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
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
import { Button } from '@/components/ui';
import { ProviderCard } from '../components/ProviderCard';
import { CustomProviderCard } from '../components/CustomProviderCard';

const EMPTY_CUSTOM_PROVIDERS: CustomOpenAIProvider[] = [];

interface AIModelsTabProps {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  customProviders: CustomOpenAIProvider[];
  providerEntries: AIServiceProvider[];
  localPreferredModel: ModelChoice;
  localFallbackModel?: ModelChoice | null;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
  onCustomProvidersChange: (providers: CustomOpenAIProvider[]) => void;
  onPreferredModelChange: (model: string, provider: string) => void;
  onFallbackModelChange: (model: string, provider: string) => void;
}

function AIModelsTabComponent({
  serviceConfigs,
  customProviders,
  providerEntries,
  localPreferredModel,
  localFallbackModel,
  onPendingChange,
  onCustomProvidersChange,
  onPreferredModelChange,
  onFallbackModelChange,
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
