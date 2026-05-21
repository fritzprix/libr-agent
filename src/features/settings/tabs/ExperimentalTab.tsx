import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/components/ui/switch';
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
    </div>
  );
}

export default React.memo(ExperimentalTabComponent);
