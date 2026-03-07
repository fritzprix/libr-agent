import React from 'react';
import { useTranslation } from 'react-i18next';
import {
  AdvancedSettings,
  DisplaySettings,
  ContextStrategy,
} from '@/context/SettingsContext';
import { Input } from '@/components/ui';

interface ChatInterfaceTabProps {
  localContextStrategy: ContextStrategy;
  localWindowSize: number;
  localToolCallGroupVisibleCount: number;
  localAdvancedSettings: AdvancedSettings;
  localDisplay: DisplaySettings;
  onContextStrategyChange: (strategy: ContextStrategy) => void;
  onWindowSizeChange: (size: number) => void;
  onToolCallGroupVisibleCountChange: (count: number) => void;
  onAdvancedSettingsChange: (
    key: keyof AdvancedSettings,
    value: number,
  ) => void;
  onDisplaySettingsChange: (
    key: keyof DisplaySettings,
    value: DisplaySettings[keyof DisplaySettings],
  ) => void;
}

function ChatInterfaceTabComponent({
  localContextStrategy,
  localWindowSize,
  localToolCallGroupVisibleCount,
  localAdvancedSettings,
  localDisplay,
  onContextStrategyChange,
  onWindowSizeChange,
  onToolCallGroupVisibleCountChange,
  onAdvancedSettingsChange,
  onDisplaySettingsChange,
}: ChatInterfaceTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      {/* Context Strategy Selector */}
      <div>
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.contextStrategy', 'Context Management Strategy')}
        </label>
        <div
          className="grid grid-cols-2 gap-3 max-w-lg"
          role="radiogroup"
          aria-label={t('settings.contextStrategy', 'Context Management Strategy')}
        >
          {(['window', 'compact'] as ContextStrategy[]).map((strategy) => {
            const isSelected = localContextStrategy === strategy;
            return (
              <button
                key={strategy}
                type="button"
                role="radio"
                aria-checked={isSelected}
                onClick={() => onContextStrategyChange(strategy)}
                className={[
                  'flex flex-col gap-1 rounded-lg border p-4 text-left transition-colors',
                  isSelected
                    ? 'border-primary bg-primary/5 text-foreground'
                    : 'border-border bg-background text-muted-foreground hover:border-primary/50',
                ].join(' ')}
              >
                <span className="font-medium text-sm">
                  {strategy === 'window'
                    ? t('settings.contextStrategy.window', 'Sliding Window')
                    : t('settings.contextStrategy.compact', 'Compact')}
                </span>
                <span className="text-xs leading-snug">
                  {strategy === 'window'
                    ? t(
                        'settings.contextStrategy.windowDescription',
                        'Keep the N most recent messages. Simple and predictable.',
                      )
                    : t(
                        'settings.contextStrategy.compactDescription',
                        'Summarize old turns and keep a recent window. Better for long sessions.',
                      )}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.messageWindowSize', 'Message Window Size')}
          </label>
          <Input
            type="number"
            placeholder="e.g., 50"
            value={localWindowSize}
            onChange={(e) =>
              onWindowSizeChange(parseInt(e.target.value, 10) || 0)
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {localContextStrategy === 'compact'
              ? t(
                  'settings.messageWindowSizeRecentDescription',
                  'Number of recent messages to keep after a compact summary',
                )
              : t(
                  'settings.messageWindowSizeDescription',
                  'Number of messages to keep in conversation history',
                )}
          </p>
        </div>

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.toolCallGroupVisibleCount',
              'Tool Calls Visible Count',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 4"
            min={1}
            max={20}
            value={localToolCallGroupVisibleCount}
            onChange={(e) =>
              onToolCallGroupVisibleCountChange(
                parseInt(e.target.value, 10) || 4,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.toolCallGroupVisibleCountDescription',
              'Number of tool calls to show in collapsed group view',
            )}
          </p>
        </div>

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.chatInterface.diffContextLines', 'Diff Context Lines')}
          </label>
          <Input
            type="number"
            placeholder="e.g., 3"
            min={1}
            max={10}
            value={localAdvancedSettings.diffContextLines ?? 3}
            onChange={(e) =>
              onAdvancedSettingsChange(
                'diffContextLines',
                parseInt(e.target.value, 10) || 3,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.chatInterface.diffContextLinesDescription',
              'Number of context lines to show in file edit diffs (1-10).',
            )}
          </p>
        </div>
      </div>

      <div className="border-t pt-6 mt-6">
        <h3 className="text-lg font-medium text-foreground mb-4">
          {t('settings.display.metricsTitle', 'Performance Metrics')}
        </h3>
        <div className="space-y-6">
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

          <div className="flex flex-col gap-4">
            <div className="min-w-0">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={localDisplay.showTokenSpeed}
                  onChange={(e) =>
                    onDisplaySettingsChange('showTokenSpeed', e.target.checked)
                  }
                  className="w-4 h-4"
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
                  className="w-4 h-4"
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

      <div className="border-t pt-6 mt-6">
        <h3 className="text-lg font-medium text-foreground mb-4">
          {t('settings.display.toolCallsTitle', 'Tool Calls')}
        </h3>
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
  );
}

export default React.memo(ChatInterfaceTabComponent, (prev, next) => {
  return (
    prev.localContextStrategy === next.localContextStrategy &&
    prev.localWindowSize === next.localWindowSize &&
    prev.localToolCallGroupVisibleCount ===
      next.localToolCallGroupVisibleCount &&
    prev.localAdvancedSettings.diffContextLines ===
      next.localAdvancedSettings.diffContextLines &&
    prev.localDisplay.metricDisplayMode ===
      next.localDisplay.metricDisplayMode &&
    prev.localDisplay.prefillDisplayFormat ===
      next.localDisplay.prefillDisplayFormat &&
    prev.localDisplay.showTokenSpeed === next.localDisplay.showTokenSpeed &&
    prev.localDisplay.compactMetrics === next.localDisplay.compactMetrics &&
    prev.localDisplay.toolDetailLevel === next.localDisplay.toolDetailLevel &&
    prev.onWindowSizeChange === next.onWindowSizeChange &&
    prev.onContextStrategyChange === next.onContextStrategyChange &&
    prev.onToolCallGroupVisibleCountChange ===
      next.onToolCallGroupVisibleCountChange &&
    prev.onAdvancedSettingsChange === next.onAdvancedSettingsChange &&
    prev.onDisplaySettingsChange === next.onDisplaySettingsChange
  );
});
