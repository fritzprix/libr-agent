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
  localAgentHubUrl: string;
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
  onPreferredModelChange: (model: string, provider: string) => void;
  onAgentHubUrlChange: (url: string) => void;
}

export function AIModelsTab({
  serviceConfigs,
  providerEntries,
  localPreferredModel,
  localAgentHubUrl,
  onPendingChange,
  onPreferredModelChange,
  onAgentHubUrlChange,
}: AIModelsTabProps) {
  const { t } = useTranslation('common');

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
            const providerName =
              provider.charAt(0).toUpperCase() + provider.slice(1);
            return (
              <ProviderCard
                key={provider}
                provider={provider}
                providerName={providerName}
                apiKey={cfg.apiKey || ''}
                baseUrl={cfg.baseUrl}
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
      </div>

      {/* Agent Hub Section */}
      <div className="space-y-4">
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.aiModels.agentHub', 'Agent Hub')}
        </h3>
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            Agent Hub URL
          </label>
          <Input
            type="url"
            placeholder="https://api.agenthub.com"
            value={localAgentHubUrl}
            onChange={(e) => onAgentHubUrlChange(e.target.value)}
            className="bg-background border text-foreground w-full"
          />
          <p className="text-xs text-muted-foreground mt-1">
            URL of the remote Agent Hub server. If set, assistants will be
            synced with this server.
          </p>
        </div>
      </div>
    </div>
  );
}
