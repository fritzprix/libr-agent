import React from 'react';
import { useTranslation } from 'react-i18next';
import { DisplaySettings } from '@/context/SettingsContext';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui';

const LANGUAGE_OPTIONS = [
  { value: 'en', labelKey: 'settings.language.en', fallback: 'English' },
  { value: 'ko', labelKey: 'settings.language.ko', fallback: '한국어' },
  { value: 'zh', labelKey: 'settings.language.zh', fallback: '简体中文' },
  { value: 'ja', labelKey: 'settings.language.ja', fallback: '日本語' },
  { value: 'fr', labelKey: 'settings.language.fr', fallback: 'Français' },
  { value: 'es', labelKey: 'settings.language.es', fallback: 'Español' },
  { value: 'de', labelKey: 'settings.language.de', fallback: 'Deutsch' },
  { value: 'pt', labelKey: 'settings.language.pt', fallback: 'Português' },
] as const;

const TOOL_DETAIL_LEVEL_OPTIONS = [
  {
    value: 'simple',
    labelKey: 'settings.display.toolDetailSimple',
    fallback: 'Simple (tool name only)',
  },
  {
    value: 'developer',
    labelKey: 'settings.display.toolDetailDeveloper',
    fallback: 'Developer (params, errors, timing)',
  },
] as const;

const METRIC_DISPLAY_MODE_OPTIONS = [
  {
    value: 'inline',
    labelKey: 'settings.display.inline',
    fallback: 'Inline (show in message)',
  },
  {
    value: 'tooltip',
    labelKey: 'settings.display.tooltip',
    fallback: 'Tooltip (hover to see)',
  },
] as const;

const PREFILL_DISPLAY_FORMAT_OPTIONS = [
  {
    value: 'time',
    labelKey: 'settings.display.time',
    fallback: 'Time to First Token (e.g., 245ms)',
  },
  {
    value: 'tokensPerSecond',
    labelKey: 'settings.display.tokensPerSecond',
    fallback: 'Tokens Per Second (e.g., 520 tok/s)',
  },
] as const;

function isToolDetailLevel(
  value: string,
): value is DisplaySettings['toolDetailLevel'] {
  return TOOL_DETAIL_LEVEL_OPTIONS.some((option) => option.value === value);
}

function isMetricDisplayMode(
  value: string,
): value is DisplaySettings['metricDisplayMode'] {
  return METRIC_DISPLAY_MODE_OPTIONS.some((option) => option.value === value);
}

function isPrefillDisplayFormat(
  value: string,
): value is DisplaySettings['prefillDisplayFormat'] {
  return PREFILL_DISPLAY_FORMAT_OPTIONS.some(
    (option) => option.value === value,
  );
}

interface GeneralTabProps {
  localLanguage: string;
  onChange: (lang: string) => void;
  localDisplay: DisplaySettings;
  onDisplaySettingsChange: (
    key: keyof DisplaySettings,
    value: DisplaySettings[keyof DisplaySettings],
  ) => void;
}

function GeneralTabComponent({
  localLanguage,
  onChange,
  localDisplay,
  onDisplaySettingsChange,
}: GeneralTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.language.label', 'Language')}
        </label>
        <Select value={localLanguage} onValueChange={onChange}>
          <SelectTrigger className="w-full max-w-xs bg-background">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {LANGUAGE_OPTIONS.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {t(option.labelKey, option.fallback)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="border-t pt-6">
        <h3 className="text-lg font-medium text-foreground mb-4">
          {t('settings.display.uiVisualsTitle', 'UI Visuals')}
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t('settings.display.fontFamily', 'Font Family')}
            </label>
            <Select
              value={localDisplay.fontFamily ?? 'Pretendard'}
              onValueChange={(value) =>
                onDisplaySettingsChange('fontFamily', value)
              }
            >
              <SelectTrigger className="w-full max-w-xs bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="Pretendard">
                  {t(
                    'settings.display.fontFamilyOptions.pretendard',
                    'Pretendard (Standard Sans)',
                  )}
                </SelectItem>
                <SelectItem value="Inter">
                  {t(
                    'settings.display.fontFamilyOptions.inter',
                    'Inter (Clean UI Sans)',
                  )}
                </SelectItem>
                <SelectItem value="NanumSquare Neo">
                  {t(
                    'settings.display.fontFamilyOptions.nanumSquareNeo',
                    'NanumSquare Neo (Modern Geometric)',
                  )}
                </SelectItem>
                <SelectItem value="D2Coding">
                  {t(
                    'settings.display.fontFamilyOptions.d2coding',
                    'D2Coding (Developer Mono)',
                  )}
                </SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.display.fontFamilyDescription',
                'Choose your preferred font for the application interface',
              )}
            </p>
          </div>

          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t('settings.display.toolDetailLevel', 'Tool Detail Level')}
            </label>
            <Select
              value={localDisplay.toolDetailLevel ?? 'simple'}
              onValueChange={(value) => {
                if (isToolDetailLevel(value)) {
                  onDisplaySettingsChange('toolDetailLevel', value);
                }
              }}
            >
              <SelectTrigger className="w-full max-w-xs bg-background">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {TOOL_DETAIL_LEVEL_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {t(option.labelKey, option.fallback)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.display.toolDetailLevelDescription',
                'Simple mode shows only tool names and status icons. Developer mode shows full parameters, error details, and execution time.',
              )}
            </p>
          </div>
        </div>
      </div>

      <div className="border-t pt-6">
        <h3 className="text-lg font-medium text-foreground mb-4">
          {t('settings.display.metricsTitle', 'Chat Metrics Display')}
        </h3>
        <div className="space-y-6">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            <div className="min-w-0">
              <label className="block text-muted-foreground mb-2 font-medium">
                {t('settings.display.metricDisplayMode', 'Metric Display Mode')}
              </label>
              <Select
                value={localDisplay.metricDisplayMode}
                onValueChange={(value) => {
                  if (isMetricDisplayMode(value)) {
                    onDisplaySettingsChange('metricDisplayMode', value);
                  }
                }}
              >
                <SelectTrigger className="w-full max-w-xs bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {METRIC_DISPLAY_MODE_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {t(option.labelKey, option.fallback)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground mt-1">
                {t(
                  'settings.display.metricDisplayModeDescription',
                  'Choose how token metrics are displayed in chat messages',
                )}
              </p>
            </div>

            <div className="min-w-0">
              <label className="block text-muted-foreground mb-2 font-medium">
                {t(
                  'settings.display.prefillDisplayFormat',
                  'Prefill Performance Format',
                )}
              </label>
              <Select
                value={localDisplay.prefillDisplayFormat}
                onValueChange={(value) => {
                  if (isPrefillDisplayFormat(value)) {
                    onDisplaySettingsChange('prefillDisplayFormat', value);
                  }
                }}
              >
                <SelectTrigger className="w-full max-w-xs bg-background">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PREFILL_DISPLAY_FORMAT_OPTIONS.map((option) => (
                    <SelectItem key={option.value} value={option.value}>
                      {t(option.labelKey, option.fallback)}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground mt-1">
                {t(
                  'settings.display.prefillDisplayFormatDescription',
                  'Choose how prefill performance is displayed',
                )}
              </p>
            </div>
          </div>

          <div className="flex flex-col gap-4">
            <div className="min-w-0">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={localDisplay.showTokenSpeed}
                  onChange={(e) =>
                    onDisplaySettingsChange('showTokenSpeed', e.target.checked)
                  }
                  className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
                />
                <span className="text-muted-foreground font-medium">
                  {t('settings.display.showTokenSpeed', 'Show Token Speed')}
                </span>
              </label>
              <p className="text-xs text-muted-foreground mt-1 ml-6">
                {t(
                  'settings.display.showTokenSpeedDescription',
                  'Display generation speed (tokens per second) in metrics',
                )}
              </p>
            </div>

            <div className="min-w-0">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={localDisplay.compactMetrics}
                  onChange={(e) =>
                    onDisplaySettingsChange('compactMetrics', e.target.checked)
                  }
                  className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
                />
                <span className="text-muted-foreground font-medium">
                  {t('settings.display.compactMetrics', 'Compact Metrics')}
                </span>
              </label>
              <p className="text-xs text-muted-foreground mt-1 ml-6">
                {t(
                  'settings.display.compactMetricsDescription',
                  'Use compact display format for token metrics',
                )}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default React.memo(GeneralTabComponent);
