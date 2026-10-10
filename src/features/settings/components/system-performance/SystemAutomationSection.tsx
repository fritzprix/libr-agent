import React from 'react';
import { useTranslation } from 'react-i18next';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { SystemPerformanceSettingsProps } from './types';

type SystemAutomationSectionProps = Pick<
  SystemPerformanceSettingsProps,
  'localSystemSettings' | 'onChange'
>;

function SystemAutomationSectionComponent({
  localSystemSettings,
  onChange,
}: SystemAutomationSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-4 rounded-xl border border-border/70 p-4">
      <h4 className="text-sm font-medium text-foreground">
        {t('settings.system.automation', 'Automation Rules')}
      </h4>

      <NumberSettingField
        label={t(
          'settings.system.scheduledTaskMinimumInterval',
          'Scheduled Task Minimum Interval (Min)',
        )}
        description={t(
          'settings.system.scheduledTaskMinimumIntervalDescription',
          'Minimum interval for scheduled tasks. Set to 0 to remove this limit.',
        )}
        placeholder={t(
          'settings.system.placeholders.scheduledTaskMinimumInterval',
          '0 = disabled',
        )}
        min={0}
        value={localSystemSettings.scheduledTaskMinimumIntervalMinutes}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 0,
            min: 0,
          })
        }
        onValueChange={(value) =>
          onChange('scheduledTaskMinimumIntervalMinutes', value)
        }
        containerClassName="min-w-0"
      />
    </div>
  );
}

export const SystemAutomationSection = React.memo(
  SystemAutomationSectionComponent,
);
