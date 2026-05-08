import React from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { SystemPerformanceSettingsProps } from './types';

type SystemNetworkSectionProps = Pick<
  SystemPerformanceSettingsProps,
  'localSystemSettings' | 'networkSettingsChanged' | 'onChange'
>;

function SystemNetworkSectionComponent({
  localSystemSettings,
  networkSettingsChanged,
  onChange,
}: SystemNetworkSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-4 rounded-xl border border-border/70 p-4">
      <div className="flex items-center gap-2">
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.system.network', 'Network')}
        </h4>
        {networkSettingsChanged && (
          <Badge
            variant="outline"
            className="border-warning/30 bg-warning/10 text-warning-foreground"
          >
            {t('settings.system.restartRequired', 'Restart required')}
          </Badge>
        )}
      </div>
      <div
        className={
          networkSettingsChanged
            ? 'rounded-lg border border-warning/30 bg-warning/10 px-3 py-2'
            : ''
        }
      >
        <p
          className={
            networkSettingsChanged
              ? 'text-xs text-warning-foreground'
              : 'text-xs text-muted-foreground'
          }
        >
          {networkSettingsChanged
            ? t(
                'settings.system.networkRestartPending',
                'You changed network settings. Save changes, then restart the app to apply them.',
              )
            : t(
                'settings.system.networkRestartNotice',
                'Changes to HTTP server network settings are applied after restarting the app.',
              )}
        </p>
      </div>

      <NumberSettingField
        label={t('settings.system.httpServerPort', 'HTTP Server Port')}
        description={t(
          'settings.system.httpServerPortDescription',
          'Port used by the internal HTTP API server.',
        )}
        placeholder={t(
          'settings.system.placeholders.httpServerPort',
          'e.g., 3030',
        )}
        min={1}
        max={65535}
        value={localSystemSettings.httpServerPort ?? 3030}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 3030,
            min: 1,
            max: 65535,
          })
        }
        onValueChange={(value) => onChange('httpServerPort', value)}
        containerClassName="min-w-0"
      />

      <div className="min-w-0">
        <label className="mb-2 block text-muted-foreground font-medium">
          {t('settings.system.httpServerExpose', 'Expose HTTP Server')}
        </label>
        <select
          value={localSystemSettings.httpServerExpose ? 'public' : 'local'}
          onChange={(event) =>
            onChange('httpServerExpose', event.target.value === 'public')
          }
          className="bg-background border text-foreground w-full max-w-xs rounded p-2"
        >
          <option value="local">
            {t(
              'settings.system.httpServerExposeMode.local',
              'Local only (127.0.0.1)',
            )}
          </option>
          <option value="public">
            {t(
              'settings.system.httpServerExposeMode.public',
              'Expose to network (0.0.0.0)',
            )}
          </option>
        </select>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.system.httpServerExposeDescription',
            'Use local-only by default. Exposing allows access from other devices on your network.',
          )}
        </p>
        {localSystemSettings.httpServerExpose && (
          <p className="mt-2 text-xs text-warning">
            {t(
              'settings.system.httpServerExposeWarning',
              'Warning: HTTP API is exposed to your network (0.0.0.0). Use only in trusted networks and protect access appropriately.',
            )}
          </p>
        )}
      </div>
    </div>
  );
}

export const SystemNetworkSection = React.memo(SystemNetworkSectionComponent);
