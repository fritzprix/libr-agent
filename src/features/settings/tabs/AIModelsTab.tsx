import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import { Input } from '@/components/ui';
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import { ProviderCard } from '../components/ProviderCard';

interface AIModelsTabProps {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  providerEntries: AIServiceProvider[];
  localPreferredModel: { provider: AIServiceProvider; model: string };
  localFallbackModel?: { provider: AIServiceProvider; model: string } | null;
  localAgentHubUrl: string;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
  onPreferredModelChange: (model: string, provider: string) => void;
  onFallbackModelChange: (model: string, provider: string) => void;
  onAgentHubUrlChange: (url: string) => void;
}

function AIModelsTabComponent({
  serviceConfigs,
  providerEntries,
  localPreferredModel,
  localFallbackModel,
  localAgentHubUrl,
  onPendingChange,
  onPreferredModelChange,
  onFallbackModelChange,
  onAgentHubUrlChange,
}: AIModelsTabProps) {
  const { t } = useTranslation('common');

  // Memoize provider names to prevent recalculation on every render
  const providerNames = useMemo(() => {
    return providerEntries.reduce(
      (acc, provider) => {
        acc[provider] = provider.charAt(0).toUpperCase() + provider.slice(1);
        return acc;
      },
      {} as Record<AIServiceProvider, string>,
    );
  }, [providerEntries]);

  return (
    <div className="space-y-8">
      {/* API Keys Section */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.apiKeys', 'Provider API Keys')}
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {providerEntries.map((provider) => {
            const cfg = serviceConfigs[provider] || {};
            return (
              <ProviderCard
                key={provider}
                provider={provider}
                providerName={providerNames[provider]}
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

        {/* Fallback Model — SP4: used when primary model fails all retries */}
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.aiModels.fallbackModel',
              'Fallback LLM (SP4 Recovery)',
            )}
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

      {/* Agent Hub Section */}
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
    </div>
  );
}

export default React.memo(AIModelsTabComponent, (prev, next) => {
  return (
    prev.localPreferredModel.provider === next.localPreferredModel.provider &&
    prev.localPreferredModel.model === next.localPreferredModel.model &&
    prev.localFallbackModel?.provider === next.localFallbackModel?.provider &&
    prev.localFallbackModel?.model === next.localFallbackModel?.model &&
    prev.localAgentHubUrl === next.localAgentHubUrl &&
    prev.onPendingChange === next.onPendingChange &&
    prev.onPreferredModelChange === next.onPreferredModelChange &&
    prev.onFallbackModelChange === next.onFallbackModelChange &&
    prev.onAgentHubUrlChange === next.onAgentHubUrlChange
  );
});
