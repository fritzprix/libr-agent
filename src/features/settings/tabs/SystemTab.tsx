import React from 'react';
import { useTranslation } from 'react-i18next';
import type { SystemSettings } from '@/context/SettingsContext';
import { SystemPerformanceSettings } from '../components/SystemPerformanceSettings';

interface SystemTabProps {
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    networkSettingsChanged: boolean;
    onChange: (
      key: keyof SystemSettings,
      value: number | string | boolean,
    ) => void;
  };
}

function SystemTabComponent({ systemSettingsProps }: SystemTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.tabs.system', 'System')}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          {t(
            'settings.system.description',
            'Control app runtime, background workers, automation limits, and network behavior.',
          )}
        </p>
      </div>
      <SystemPerformanceSettings {...systemSettingsProps} />
    </div>
  );
}

export default React.memo(SystemTabComponent, (prev, next) => {
  return (
    prev.systemSettingsProps.networkSettingsChanged ===
      next.systemSettingsProps.networkSettingsChanged &&
    prev.systemSettingsProps.localSystemSettings ===
      next.systemSettingsProps.localSystemSettings &&
    prev.systemSettingsProps.onChange === next.systemSettingsProps.onChange
  );
});
