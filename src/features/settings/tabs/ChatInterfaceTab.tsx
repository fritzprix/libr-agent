import React from 'react';
import { useTranslation } from 'react-i18next';
import { AdvancedSettings, ContextStrategy } from '@/context/SettingsContext';
import { Button, Input, Slider } from '@/components/ui';

interface ChatInterfaceTabProps {
  localContextStrategy: ContextStrategy;
  localWindowSize: number;
  localMaxInputContext: number;
  localToolCallGroupVisibleCount: number;
  localAdvancedSettings: AdvancedSettings;
  onContextStrategyChange: (strategy: ContextStrategy) => void;
  onWindowSizeChange: (size: number) => void;
  onMaxInputContextChange: (size: number) => void;
  onToolCallGroupVisibleCountChange: (count: number) => void;
  onAdvancedSettingsChange: (
    key: keyof AdvancedSettings,
    value: number,
  ) => void;
}

function ChatInterfaceTabComponent({
  localContextStrategy,
  localWindowSize,
  localMaxInputContext,
  localToolCallGroupVisibleCount,
  localAdvancedSettings,
  onContextStrategyChange,
  onWindowSizeChange,
  onMaxInputContextChange,
  onToolCallGroupVisibleCountChange,
  onAdvancedSettingsChange,
}: ChatInterfaceTabProps) {
  const { t } = useTranslation('common');

  const updateToolCallCount = (delta: number) => {
    onToolCallGroupVisibleCountChange(
      Math.min(20, Math.max(1, localToolCallGroupVisibleCount + delta)),
    );
  };

  const updateDiffContextLines = (delta: number) => {
    onAdvancedSettingsChange(
      'diffContextLines',
      Math.min(
        10,
        Math.max(1, (localAdvancedSettings.diffContextLines ?? 3) + delta),
      ),
    );
  };

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
          aria-label={t(
            'settings.contextStrategy',
            'Context Management Strategy',
          )}
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
                  'flex flex-col gap-1 rounded-lg border p-4 text-left transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
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
        {localContextStrategy === 'window' ? (
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
              {t(
                'settings.messageWindowSizeDescription',
                'Number of messages to keep in conversation history',
              )}
            </p>
          </div>
        ) : (
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-4 font-medium">
              {t('settings.maxInputContext', 'Max Input Context')}
            </label>
            <div className="flex items-center gap-4 max-w-xs">
              <Slider
                min={8192}
                max={262144}
                step={8192}
                value={[localMaxInputContext]}
                onValueChange={([val]) => onMaxInputContextChange(val)}
                className="flex-1"
              />
              <span className="text-sm font-mono text-primary min-w-[3.5rem] text-right">
                {Math.round(localMaxInputContext / 1024)}K
              </span>
            </div>
            <p className="text-xs text-muted-foreground mt-4">
              {t(
                'settings.maxInputContextDescription',
                'Maximum token count before summarizing old turns. Higher values keep more detail but increase cost.',
              )}
            </p>
          </div>
        )}

        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.toolCallGroupVisibleCount',
              'Tool Calls Visible Count',
            )}
          </label>
          <div className="flex max-w-xs items-center gap-2">
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() => updateToolCallCount(-1)}
              disabled={localToolCallGroupVisibleCount <= 1}
              aria-label={t('settings.decreaseToolCallCount', 'Decrease tool call count')}
            >
              -
            </Button>
            <div className="flex h-9 min-w-[4rem] items-center justify-center rounded-md border bg-background px-3 text-sm font-medium">
              {localToolCallGroupVisibleCount}
            </div>
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() => updateToolCallCount(1)}
              disabled={localToolCallGroupVisibleCount >= 20}
              aria-label={t('settings.increaseToolCallCount', 'Increase tool call count')}
            >
              +
            </Button>
          </div>
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
          <div className="flex max-w-xs items-center gap-2">
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() => updateDiffContextLines(-1)}
              disabled={(localAdvancedSettings.diffContextLines ?? 3) <= 1}
              aria-label={t('settings.chatInterface.decreaseDiffLines', 'Decrease diff context lines')}
            >
              -
            </Button>
            <div className="flex h-9 min-w-[4rem] items-center justify-center rounded-md border bg-background px-3 text-sm font-medium">
              {localAdvancedSettings.diffContextLines ?? 3}
            </div>
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() => updateDiffContextLines(1)}
              disabled={(localAdvancedSettings.diffContextLines ?? 3) >= 10}
              aria-label={t('settings.chatInterface.increaseDiffLines', 'Increase diff context lines')}
            >
              +
            </Button>
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.chatInterface.diffContextLinesDescription',
              'Number of context lines to show in file edit diffs (1-10).',
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
    prev.localMaxInputContext === next.localMaxInputContext &&
    prev.localToolCallGroupVisibleCount ===
      next.localToolCallGroupVisibleCount &&
    prev.localAdvancedSettings.diffContextLines ===
      next.localAdvancedSettings.diffContextLines &&
    prev.onWindowSizeChange === next.onWindowSizeChange &&
    prev.onContextStrategyChange === next.onContextStrategyChange &&
    prev.onMaxInputContextChange === next.onMaxInputContextChange &&
    prev.onToolCallGroupVisibleCountChange ===
      next.onToolCallGroupVisibleCountChange &&
    prev.onAdvancedSettingsChange === next.onAdvancedSettingsChange
  );
});
