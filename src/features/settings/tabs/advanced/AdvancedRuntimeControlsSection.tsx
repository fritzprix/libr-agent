import React from 'react';
import { useTranslation } from 'react-i18next';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { AdvancedSettingsSectionProps } from './types';
import {
  REPEATED_THINKING_MIN_PATTERN_LENGTH,
  REPEATED_THINKING_MIN_REPETITIONS,
} from '@/context/llm/repeatedTailDetector';

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
          'settings.advanced.loopPreventionHardBreakOffset',
          'Hard Break Offset',
        )}
        description={t(
          'settings.advanced.loopPreventionHardBreakOffsetDescription',
          'Additional identical tool calls allowed after natural recovery warning before hard-stopping the workflow. With threshold=3 and offset=1, natural recovery fires at call 3 and hard break at call 4.',
        )}
        placeholder={t(
          'settings.advanced.loopPreventionHardBreakOffsetPlaceholder',
          'e.g., 1',
        )}
        min={1}
        max={20}
        step={1}
        value={localAdvancedSettings.loopPreventionHardBreakOffset ?? 2}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 1,
            min: 1,
            max: 20,
          })
        }
        onValueChange={(value) =>
          onChange('loopPreventionHardBreakOffset', value)
        }
      />

      <NumberSettingField
        label={t(
          'settings.advanced.thinkingLoopMinPatternLength',
          'Thinking Loop Min Pattern Length',
        )}
        description={t(
          'settings.advanced.thinkingLoopMinPatternLengthDescription',
          'Minimum repeating string length required to trigger a thinking loop detection. Larger values avoid false positives during long reasoning paths.',
        )}
        placeholder={t(
          'settings.advanced.thinkingLoopMinPatternLengthPlaceholder',
          'e.g., 256',
        )}
        min={32}
        max={1024}
        step={1}
        value={
          localAdvancedSettings.thinkingLoopMinPatternLength ??
          REPEATED_THINKING_MIN_PATTERN_LENGTH
        }
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: REPEATED_THINKING_MIN_PATTERN_LENGTH,
            min: 32,
            max: 1024,
          })
        }
        onValueChange={(value) =>
          onChange('thinkingLoopMinPatternLength', value)
        }
      />

      <NumberSettingField
        label={t(
          'settings.advanced.thinkingLoopMinRepetitions',
          'Thinking Loop Min Repetitions',
        )}
        description={t(
          'settings.advanced.thinkingLoopMinRepetitionsDescription',
          'Minimum number of times a repeating pattern must occur in the thinking block stream to trigger a thinking loop detection.',
        )}
        placeholder={t(
          'settings.advanced.thinkingLoopMinRepetitionsPlaceholder',
          'e.g., 4',
        )}
        min={2}
        max={10}
        step={1}
        value={
          localAdvancedSettings.thinkingLoopMinRepetitions ??
          REPEATED_THINKING_MIN_REPETITIONS
        }
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: REPEATED_THINKING_MIN_REPETITIONS,
            min: 2,
            max: 10,
          })
        }
        onValueChange={(value) => onChange('thinkingLoopMinRepetitions', value)}
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
          'settings.advanced.defaultSessionMaxDepthPlaceholder',
          'e.g., 3',
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
