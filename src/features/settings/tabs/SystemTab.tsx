import React from 'react';
import { useTranslation } from 'react-i18next';
import type { SystemSettings } from '@/context/SettingsContext';
import { SystemPerformanceSettings } from '../components/SystemPerformanceSettings';
import { Button, Slider } from '@/components/ui';

interface SystemTabProps {
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    networkSettingsChanged: boolean;
    onChange: (
      key: keyof SystemSettings,
      value: string | number | boolean,
    ) => void;
  };
}

const STORAGE_PRESETS_MB = [10, 25, 50, 100, 250, 500] as const;

function findNearestStorageIndex(value: number): number {
  return STORAGE_PRESETS_MB.reduce((bestIndex, preset, index) => {
    const bestDistance = Math.abs(STORAGE_PRESETS_MB[bestIndex] - value);
    const nextDistance = Math.abs(preset - value);
    return nextDistance < bestDistance ? index : bestIndex;
  }, 0);
}

function SystemTabComponent({ systemSettingsProps }: SystemTabProps) {
  const { t } = useTranslation('common');
  const { localSystemSettings, onChange } = systemSettingsProps;

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

      <div className="border-t pt-6">
        <h3 className="mb-4 text-lg font-medium text-foreground">
          {t('settings.system.fileWorkspace', 'File & Workspace')}
        </h3>

        <div className="space-y-6">
          <div className="min-w-0 rounded-xl border border-border/70 p-4 max-w-lg">
            <div className="mb-4 flex items-center justify-between gap-3">
              <label className="block text-muted-foreground font-medium">
                {t(
                  'settings.system.maxFileUploadSize',
                  'Max File Upload Size (MB)',
                )}
              </label>
              <span className="rounded-md bg-primary/10 px-2 py-1 text-sm font-mono text-primary">
                {`${localSystemSettings.maxFileUploadSizeMB} MB`}
              </span>
            </div>
            <Slider
              min={0}
              max={STORAGE_PRESETS_MB.length - 1}
              step={1}
              value={[
                findNearestStorageIndex(
                  localSystemSettings.maxFileUploadSizeMB,
                ),
              ]}
              onValueChange={([index]) =>
                onChange('maxFileUploadSizeMB', STORAGE_PRESETS_MB[index] ?? 50)
              }
            />
            <div className="mt-3 flex flex-wrap gap-2">
              {STORAGE_PRESETS_MB.map((preset) => (
                <Button
                  key={preset}
                  type="button"
                  variant={
                    preset === localSystemSettings.maxFileUploadSizeMB
                      ? 'default'
                      : 'outline'
                  }
                  className="h-8 px-2 text-xs"
                  onClick={() => onChange('maxFileUploadSizeMB', preset)}
                >
                  {`${preset} MB`}
                </Button>
              ))}
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              {t(
                'settings.system.maxFileUploadSizeDescription',
                'Maximum size for a single file upload. Increase if you often work with large documents.',
              )}
            </p>
          </div>
        </div>
      </div>

      <div className="border-t pt-6">
        <SystemPerformanceSettings {...systemSettingsProps} />
      </div>
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
