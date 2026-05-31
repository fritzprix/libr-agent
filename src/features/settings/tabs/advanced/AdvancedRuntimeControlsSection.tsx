import React from 'react';
import { useTranslation } from 'react-i18next';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { AdvancedSettingsSectionProps } from './types';

function AdvancedRuntimeControlsSectionComponent({
  localAdvancedSettings,
  onChange,
}: AdvancedSettingsSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
      <NumberSettingField
        label={t(
          'settings.advanced.loopPreventionThreshold',
          'Loop Prevention Threshold',
        )}
        description={t(
          'settings.advanced.loopPreventionThresholdDescription',
          'Number of repeated identical tool outcomes before the agent attempts natural recovery or triggers a hard stop.',
        )}
        placeholder={t(
          'settings.advanced.loopPreventionThresholdPlaceholder',
          'e.g., 3',
        )}
        min={2}
        max={20}
        step={1}
        value={localAdvancedSettings.loopPreventionThreshold ?? 3}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 3,
            min: 2,
            max: 20,
          })
        }
        onValueChange={(value) => onChange('loopPreventionThreshold', value)}
      />

      <NumberSettingField
        label={t(
          'settings.advanced.defaultSessionMaxDepth',
          'Session Branching Limit (Advanced)',
        )}
        description={t(
          'settings.advanced.defaultSessionMaxDepthDescription',
          'Controls how many child-session levels are allowed by default. Set 0 for unlimited. Most users can leave this as-is.',
        )}
        placeholder={t(
          'settings.advanced.defaultSessionMaxFanoutPlaceholder',
          '0 = unlimited',
        )}
        min={0}
        max={64}
        step={1}
        value={localAdvancedSettings.defaultSessionMaxDepth ?? 0}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 0,
            min: 0,
            max: 64,
          })
        }
        onValueChange={(value) => onChange('defaultSessionMaxDepth', value)}
      />

      <NumberSettingField
        label={t(
          'settings.advanced.defaultSessionMaxFanout',
          'Session Child Limit (Advanced)',
        )}
        description={t(
          'settings.advanced.defaultSessionMaxFanoutDescription',
          'Controls how many direct child sessions each parent can create by default. Set 0 for unlimited. Most users can leave this as-is.',
        )}
        placeholder={t(
          'settings.advanced.defaultSessionMaxFanoutPlaceholder',
          '0 = unlimited',
        )}
        min={0}
        max={64}
        step={1}
        value={localAdvancedSettings.defaultSessionMaxFanout ?? 0}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 0,
            min: 0,
            max: 64,
          })
        }
        onValueChange={(value) => onChange('defaultSessionMaxFanout', value)}
      />
    </div>
  );
}

export const AdvancedRuntimeControlsSection = React.memo(
  AdvancedRuntimeControlsSectionComponent,
);
