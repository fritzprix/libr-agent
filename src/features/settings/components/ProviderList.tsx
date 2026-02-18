import React, { useMemo } from 'react';
import { AIServiceProvider } from '@/lib/ai-service';
import { ServiceConfig } from '@/context/SettingsContext';
import { ProviderCard } from './ProviderCard';
import { useTranslation } from 'react-i18next';

interface ProviderListProps {
  serviceConfigs: Record<AIServiceProvider, ServiceConfig>;
  providerEntries: AIServiceProvider[];
  onPendingChange: (
    provider: AIServiceProvider,
    patch: Partial<ServiceConfig>,
  ) => void;
}

const ProviderListComponent: React.FC<ProviderListProps> = ({
  serviceConfigs,
  providerEntries,
  onPendingChange,
}) => {
  const { t } = useTranslation('common');

  // Memoize provider names to prevent recalculation
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
  );
};

export const ProviderList = React.memo(ProviderListComponent);
