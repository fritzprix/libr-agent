import { useTranslation } from 'react-i18next';
import { AdvancedSettings, SystemSettings } from '@/context/SettingsContext';
import { Input } from '@/components/ui';
import { SystemPerformanceSettings } from '../components/SystemPerformanceSettings';
import { DangerZoneSettings } from '../components/DangerZoneSettings';

interface AdvancedTabProps {
  localAdvancedSettings: AdvancedSettings;
  onChange: (key: keyof AdvancedSettings, value: number) => void;
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    onChange: (key: keyof SystemSettings, value: number) => void;
  };
  dangerZoneProps: {
    isDeleting: boolean;
    isResetting: boolean;
    onDelete: () => Promise<void>;
    onReset: () => void;
  };
}

export function AdvancedTab({
  localAdvancedSettings,
  onChange,
  systemSettingsProps,
  dangerZoneProps,
}: AdvancedTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.maxRetries', 'Max Retry Attempts')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 1"
          min={0}
          max={5}
          value={localAdvancedSettings.maxRetries}
          onChange={(e) =>
            onChange('maxRetries', parseInt(e.target.value, 10) || 0)
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.maxRetriesDescription',
            'Maximum number of retries for failed AI requests.',
          )}
        </p>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.retryDelay', 'Retry Delay (ms)')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 5000"
          min={1000}
          step={1000}
          value={localAdvancedSettings.retryDelay}
          onChange={(e) =>
            onChange('retryDelay', parseInt(e.target.value, 10) || 5000)
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.retryDelayDescription',
            'Delay in milliseconds between retry attempts.',
          )}
        </p>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.circuitBreaker', 'Tool Loop Threshold')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 3"
          min={1}
          max={10}
          value={localAdvancedSettings.circuitBreakerThreshold}
          onChange={(e) =>
            onChange(
              'circuitBreakerThreshold',
              parseInt(e.target.value, 10) || 3,
            )
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.circuitBreakerDescription',
            'Number of repeated errors or tool calls before triggering the circuit breaker.',
          )}
        </p>
      </div>

      <SystemPerformanceSettings {...systemSettingsProps} />
      <DangerZoneSettings {...dangerZoneProps} />
    </div>
  );
}
