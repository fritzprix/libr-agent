import React from 'react';
import { useTranslation } from 'react-i18next';
import { IsolationLevel } from '@/lib/services/settings-service';
import type { AdvancedSystemSettingsProps } from './types';

interface AdvancedShellIsolationSectionProps {
  systemSettingsProps: AdvancedSystemSettingsProps;
}

function AdvancedShellIsolationSectionComponent({
  systemSettingsProps,
}: AdvancedShellIsolationSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="min-w-0 rounded-xl border border-border/70 p-4">
      <label className="mb-2 block text-muted-foreground font-medium">
        {t('settings.advanced.shellIsolation', 'Shell Isolation Level')}
      </label>
      <select
        value={systemSettingsProps.localSystemSettings.shellIsolationLevel}
        onChange={(event) =>
          systemSettingsProps.onChange(
            'shellIsolationLevel',
            event.target.value as IsolationLevel,
          )
        }
        className="bg-background border text-foreground w-full max-w-xs rounded p-2"
      >
        <option value="basic">
          {t(
            'settings.advanced.shellIsolationModes.basic',
            'Basic - Full PATH access (less secure)',
          )}
        </option>
        <option value="medium">
          {t(
            'settings.advanced.shellIsolationModes.medium',
            'Medium - Restricted PATH (balanced)',
          )}
        </option>
        <option value="high">
          {t(
            'settings.advanced.shellIsolationModes.high',
            'High - Sandboxed (most secure)',
          )}
        </option>
      </select>
      <p className="mt-1 text-xs text-muted-foreground">
        {t(
          'settings.advanced.shellIsolationDescription',
          'Controls environment isolation for shell commands. Basic allows user-installed tools, High provides maximum security.',
        )}
      </p>
    </div>
  );
}

export const AdvancedShellIsolationSection = React.memo(
  AdvancedShellIsolationSectionComponent,
);
