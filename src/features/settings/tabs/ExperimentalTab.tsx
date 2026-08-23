import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/components/ui/switch';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { ExperimentalSettings } from '@/context/SettingsContext';

interface ExperimentalTabProps {
  localExperimentalSettings: ExperimentalSettings;
  onChange: <K extends keyof ExperimentalSettings>(
    key: K,
    value: ExperimentalSettings[K],
  ) => void;
}

function ExperimentalTabComponent({
  localExperimentalSettings,
  onChange,
}: ExperimentalTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.experimental.title', 'Experimental Features')}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          {t(
            'settings.experimental.summary',
            'These features are in development. Turn them on to preview early capabilities, but keep in mind they might be unstable.',
          )}
        </p>
      </div>

      <div className="rounded-xl border border-border/60 bg-card p-6 shadow-xs backdrop-blur-md">
        <div className="flex items-center justify-between gap-4">
          <div className="flex-1 space-y-1">
            <label
              className="text-sm font-medium text-foreground cursor-pointer"
              htmlFor="inline-audio-attachment"
            >
              {t(
                'settings.experimental.inlineAudioAttachment.title',
                'Inline Audio Attachments',
              )}
            </label>
            <p className="text-xs text-muted-foreground leading-normal max-w-2xl">
              {t(
                'settings.experimental.inlineAudioAttachment.description',
                'Convert attached audio files into base64 inline content for LLM requests. Disabling this saves significant context window tokens by keeping audio files as workspace-only references.',
              )}
            </p>
          </div>
          <Switch
            id="inline-audio-attachment"
            checked={localExperimentalSettings.inlineAudioAttachment}
            onCheckedChange={(checked) =>
              onChange('inlineAudioAttachment', checked)
            }
          />
        </div>
      </div>

      <div className="rounded-xl border border-border/60 bg-card p-6 shadow-xs backdrop-blur-md">
        <div className="space-y-1">
          <p className="text-sm font-medium text-foreground">
            {t(
              'settings.experimental.toolLoopRecoveryPolicy.title',
              'Tool-loop recovery',
            )}
          </p>
          <p className="text-xs text-muted-foreground leading-normal max-w-2xl">
            {t(
              'settings.experimental.toolLoopRecoveryPolicy.description',
              'By default, repeated tool loops trigger a clean resample (no intrusive guidance text). Budget exhausted → circuit breaker.',
            )}
          </p>
        </div>

        <div className="mt-4 flex items-center justify-between gap-4">
          <div className="flex-1 space-y-1">
            <label
              className="text-sm font-medium text-foreground cursor-pointer"
              htmlFor="tool-loop-legacy-guidance"
            >
              {t(
                'settings.experimental.toolLoopLegacyGuidanceEnabled.title',
                'Show loop warnings in tool results (legacy)',
              )}
            </label>
            <p className="text-xs text-muted-foreground leading-normal max-w-2xl">
              {t(
                'settings.experimental.toolLoopLegacyGuidanceEnabled.description',
                'Off by default. When enabled, injects loop-prevention guidance as tool errors instead of silently retrying with a clean resample.',
              )}
            </p>
          </div>
          <Switch
            id="tool-loop-legacy-guidance"
            checked={
              localExperimentalSettings.toolLoopRecoveryPolicy ===
              'legacyGuidance'
            }
            onCheckedChange={(checked) =>
              onChange(
                'toolLoopRecoveryPolicy',
                checked ? 'legacyGuidance' : 'resampleThenBreak',
              )
            }
          />
        </div>

        {localExperimentalSettings.toolLoopRecoveryPolicy ===
        'resampleThenBreak' ? (
          <div className="mt-4">
            <NumberSettingField
              label={t(
                'settings.experimental.toolLoopMaxResampleRetries.title',
                'Max resample retries',
              )}
              description={t(
                'settings.experimental.toolLoopMaxResampleRetries.description',
                'How many clean resample attempts before hard-stopping when repeated tool-loop signatures are detected.',
              )}
              placeholder={t(
                'settings.experimental.toolLoopMaxResampleRetries.placeholder',
                'e.g., 2',
              )}
              min={0}
              max={20}
              step={1}
              value={localExperimentalSettings.toolLoopMaxResampleRetries}
              parseValue={(rawValue) =>
                parseIntegerInput(rawValue, {
                  fallback: 2,
                  min: 0,
                  max: 20,
                })
              }
              onValueChange={(value) =>
                onChange('toolLoopMaxResampleRetries', value)
              }
            />
          </div>
        ) : null}
      </div>
    </div>
  );
}

export default React.memo(ExperimentalTabComponent);
