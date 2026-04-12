import React from 'react';
import { useTranslation } from 'react-i18next';
import { DisplaySettings } from '@/context/SettingsContext';

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
        <select
          className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
          value={localLanguage}
          onChange={(e) => onChange(e.target.value)}
        >
          <option value="en">{t('settings.language.en', 'English')}</option>
          <option value="ko">{t('settings.language.ko', '한국어')}</option>
          <option value="zh">{t('settings.language.zh', '简体中文')}</option>
          <option value="ja">{t('settings.language.ja', '日本語')}</option>
          <option value="fr">{t('settings.language.fr', 'Français')}</option>
          <option value="es">{t('settings.language.es', 'Español')}</option>
          <option value="de">{t('settings.language.de', 'Deutsch')}</option>
          <option value="pt">{t('settings.language.pt', 'Português')}</option>
        </select>
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
            <select
              className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
              value={localDisplay.fontFamily ?? 'Pretendard'}
              onChange={(e) =>
                onDisplaySettingsChange('fontFamily', e.target.value)
              }
            >
              <option value="Pretendard">Pretendard (Standard Sans)</option>
              <option value="Inter">Inter (Clean UI Sans)</option>
              <option value="NanumSquare Neo">
                NanumSquare Neo (Modern Geometric)
              </option>
              <option value="D2Coding">D2Coding (Developer Mono)</option>
            </select>
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
            <select
              className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
              value={localDisplay.toolDetailLevel ?? 'simple'}
              onChange={(e) =>
                onDisplaySettingsChange(
                  'toolDetailLevel',
                  e.target.value as 'simple' | 'developer',
                )
              }
            >
              <option value="simple">
                {t(
                  'settings.display.toolDetailSimple',
                  'Simple (tool name only)',
                )}
              </option>
              <option value="developer">
                {t(
                  'settings.display.toolDetailDeveloper',
                  'Developer (params, errors, timing)',
                )}
              </option>
            </select>
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
              <select
                className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
                value={localDisplay.metricDisplayMode}
                onChange={(e) =>
                  onDisplaySettingsChange(
                    'metricDisplayMode',
                    e.target.value as 'tooltip' | 'inline',
                  )
                }
              >
                <option value="inline">
                  {t('settings.display.inline', 'Inline (show in message)')}
                </option>
                <option value="tooltip">
                  {t('settings.display.tooltip', 'Tooltip (hover to see)')}
                </option>
              </select>
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
              <select
                className="bg-background border text-foreground rounded px-3 py-2 w-full max-w-xs"
                value={localDisplay.prefillDisplayFormat}
                onChange={(e) =>
                  onDisplaySettingsChange(
                    'prefillDisplayFormat',
                    e.target.value as 'time' | 'tokensPerSecond',
                  )
                }
              >
                <option value="time">
                  {t(
                    'settings.display.time',
                    'Time to First Token (e.g., 245ms)',
                  )}
                </option>
                <option value="tokensPerSecond">
                  {t(
                    'settings.display.tokensPerSecond',
                    'Tokens Per Second (e.g., 520 tok/s)',
                  )}
                </option>
              </select>
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

export default React.memo(GeneralTabComponent, (prev, next) => {
  return (
    prev.localLanguage === next.localLanguage &&
    prev.localDisplay.fontFamily === next.localDisplay.fontFamily &&
    prev.onChange === next.onChange &&
    prev.onDisplaySettingsChange === next.onDisplaySettingsChange
  );
});
