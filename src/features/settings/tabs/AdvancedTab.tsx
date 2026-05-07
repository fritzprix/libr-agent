import React from 'react';
import { useTranslation } from 'react-i18next';
import { DangerZoneSettings } from '../components/DangerZoneSettings';
import { AboutSection } from '../components/AboutSection';
import { AdvancedPerformanceSection } from './advanced/AdvancedPerformanceSection';
import { AdvancedRuntimeControlsSection } from './advanced/AdvancedRuntimeControlsSection';
import { AdvancedShellIsolationSection } from './advanced/AdvancedShellIsolationSection';
import type { AdvancedTabProps } from './advanced/types';

function AdvancedTabComponent({
  localAdvancedSettings,
  onChange,
  systemSettingsProps,
  dangerZoneProps,
}: AdvancedTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.advanced.title', 'Advanced Runtime Controls')}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          {t(
            'settings.advanced.summary',
            'These settings change runtime safety rails, multi-agent limits, and shell isolation. Most users should leave them alone.',
          )}
        </p>
      </div>

      <AdvancedRuntimeControlsSection
        localAdvancedSettings={localAdvancedSettings}
        onChange={onChange}
      />
      <AdvancedPerformanceSection
        localAdvancedSettings={localAdvancedSettings}
        onChange={onChange}
      />
      <AdvancedShellIsolationSection
        systemSettingsProps={systemSettingsProps}
      />

      <AboutSection />
      <DangerZoneSettings {...dangerZoneProps} />
    </div>
  );
}

export default React.memo(AdvancedTabComponent);
