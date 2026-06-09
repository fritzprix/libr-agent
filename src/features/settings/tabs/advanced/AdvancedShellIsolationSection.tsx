import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/components/ui/switch';
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
    <div className="flex min-w-0 flex-col gap-4">
      <div className="rounded-xl border border-border/70 p-4">
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

      <div className="rounded-xl border border-border/70 p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <label
              htmlFor="shell-runtime-bootstrap"
              className="block text-muted-foreground font-medium"
            >
              {t(
                'settings.advanced.shellRuntimeBootstrap',
                'Shell runtime bootstrap',
              )}
            </label>
            <p className="mt-1 text-xs text-muted-foreground">
              {t(
                'settings.advanced.shellRuntimeBootstrapDescription',
                'When enabled, persistent shells source conda.sh and nvm.sh only (not your full shell rc). PATH is extended without running conda activate.',
              )}
            </p>
          </div>
          <Switch
            id="shell-runtime-bootstrap"
            checked={
              systemSettingsProps.localSystemSettings.shellRuntimeBootstrap ??
              false
            }
            onCheckedChange={(checked) =>
              systemSettingsProps.onChange('shellRuntimeBootstrap', checked)
            }
          />
        </div>
      </div>
    </div>
  );
}

export const AdvancedShellIsolationSection = React.memo(
  AdvancedShellIsolationSectionComponent,
);
