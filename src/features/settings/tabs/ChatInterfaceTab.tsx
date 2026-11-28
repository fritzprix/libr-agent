import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AdvancedSettings, ContextStrategy } from '@/context/SettingsContext';
import { Button, Input, Slider } from '@/components/ui';
import {
  SLIDER_MAX_POSITION,
  TOKEN_UNIT,
  clampAndRoundTokenValue,
  formatTokenValue,
  sliderPositionToValue,
  valueToSliderPosition,
} from './tokenSlider';

const OUTPUT_TOKEN_PRESETS = [
  1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072,
] as const;

// Max Input Context choices: powers of two from 8K to 4M (room for 1M-class
// models; the model catalog lists 2M windows), plus 48K so the default is exact.
// A stored value that is not a preset still shows and saves unchanged until the
// user picks another one.
const MAX_INPUT_CONTEXT_PRESETS = [
  8192, 16384, 32768, 49152, 65536, 131072, 262144, 524288, 1048576, 2097152,
  4194304,
] as const;

interface TokenSliderFieldProps {
  id: string;
  label: string;
  description: string;
  value: number;
  presets: readonly number[];
  onChange: (value: number) => void;
}

function TokenSliderField({
  id,
  label,
  description,
  value,
  presets,
  onChange,
}: TokenSliderFieldProps) {
  const min = presets[0];
  const max = presets[presets.length - 1];
  const [draftValue, setDraftValue] = useState(() => String(value));

  useEffect(() => {
    setDraftValue(String(value));
  }, [value]);

  const sliderPosition = valueToSliderPosition(value, min, max);

  const commitDraft = () => {
    const parsed = Number(draftValue.trim());
    if (draftValue.trim() === '' || !Number.isFinite(parsed)) {
      setDraftValue(String(value));
      return;
    }
    const nextValue = clampAndRoundTokenValue(parsed, min, max);
    setDraftValue(String(nextValue));
    if (nextValue !== value) {
      onChange(nextValue);
    }
  };

  return (
    <div className="min-w-0 rounded-xl border border-border/70 p-4">
      <div className="mb-4 flex items-center justify-between gap-3">
        <label
          htmlFor={id}
          className="block text-muted-foreground font-medium"
        >
          {label}
        </label>
        <span className="rounded-md bg-primary/10 px-2 py-1 text-sm font-mono text-primary">
          {formatTokenValue(value)}
        </span>
      </div>
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
        <Slider
          min={0}
          max={SLIDER_MAX_POSITION}
          step={1}
          value={[sliderPosition]}
          aria-label={label}
          onValueChange={([position]) => {
            const nextValue = sliderPositionToValue(position, min, max);
            if (nextValue !== value) {
              onChange(nextValue);
            }
          }}
          className="w-full max-w-xl flex-1"
        />
        <Input
          id={id}
          type="number"
          min={min}
          max={max}
          step={TOKEN_UNIT}
          value={draftValue}
          aria-label={label}
          onChange={(e) => setDraftValue(e.target.value)}
          onBlur={commitDraft}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              commitDraft();
            } else if (e.key === 'Escape') {
              setDraftValue(String(value));
            }
          }}
          className="w-32 border bg-background font-mono text-sm text-foreground"
        />
      </div>
      <div className="mt-3 flex flex-wrap gap-2">
        {presets.map((preset) => (
          <Button
            key={preset}
            type="button"
            variant={preset === value ? 'default' : 'outline'}
            className="h-8 px-2 text-xs"
            onClick={() => onChange(preset)}
          >
            {formatTokenValue(preset)}
          </Button>
        ))}
      </div>
      <p className="mt-3 text-xs text-muted-foreground">{description}</p>
    </div>
  );
}

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
  const defaultMaxOutputTokens =
    localAdvancedSettings.defaultMaxOutputTokens ?? 8192;

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
      <div>
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.contextStrategyLabel', 'Context Management Strategy')}
        </label>
        <div
          className="grid grid-cols-2 gap-3 max-w-lg"
          role="radiogroup"
          aria-label={t(
            'settings.contextStrategyLabel',
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
                        'Keep only the last N messages.',
                      )
                    : t(
                        'settings.contextStrategy.compactDescription',
                        'Summarize older messages to keep the conversation history in long sessions.',
                      )}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <TokenSliderField
        id="settings-max-output-tokens"
        label={t(
          'settings.advanced.maxOutputTokens',
          'Max Output Tokens (Default)',
        )}
        description={t(
          'settings.advanced.maxOutputTokensDescription',
          'Default maximum output tokens when the assistant does not set its own. Read on every request.',
        )}
        value={defaultMaxOutputTokens}
        presets={OUTPUT_TOKEN_PRESETS}
        onChange={(nextValue) =>
          onAdvancedSettingsChange('defaultMaxOutputTokens', nextValue)
        }
      />

      {localContextStrategy === 'compact' && (
        <TokenSliderField
          id="settings-max-input-context"
          label={t('settings.maxInputContext', 'Max Input Context')}
          description={t(
            'settings.maxInputContextDescription',
            'Maximum token limit before summarizing older messages. Higher values keep more detail but increase cost.',
          )}
          value={localMaxInputContext}
          presets={MAX_INPUT_CONTEXT_PRESETS}
          onChange={onMaxInputContextChange}
        />
      )}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {localContextStrategy === 'window' && (
          <div className="min-w-0">
            <label className="mb-2 block font-medium text-muted-foreground">
              {t('settings.messageWindowSize', 'Message Window Size')}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.messageWindowSizePlaceholder',
                'e.g., 50',
              )}
              value={localWindowSize}
              onChange={(e) =>
                onWindowSizeChange(parseInt(e.target.value, 10) || 0)
              }
              className="w-full max-w-xs border bg-background text-foreground"
            />
            <p className="mt-1 text-xs text-muted-foreground">
              {t(
                'settings.messageWindowSizeDescription',
                'Number of messages to keep in conversation history',
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
              aria-label={t(
                'settings.decreaseToolCallCount',
                'Decrease tool call count',
              )}
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
              aria-label={t(
                'settings.increaseToolCallCount',
                'Increase tool call count',
              )}
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
            {t(
              'settings.chatInterface.diffContextLines',
              'Code Diff Context Lines',
            )}
          </label>
          <div className="flex max-w-xs items-center gap-2">
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() => updateDiffContextLines(-1)}
              disabled={(localAdvancedSettings.diffContextLines ?? 3) <= 1}
              aria-label={t(
                'settings.chatInterface.decreaseDiffLines',
                'Decrease diff context lines',
              )}
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
              aria-label={t(
                'settings.chatInterface.increaseDiffLines',
                'Increase diff context lines',
              )}
            >
              +
            </Button>
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.chatInterface.diffContextLinesDescription',
              'Number of unchanged lines to show around changes (1-10).',
            )}
          </p>
        </div>
      </div>
    </div>
  );
}

export default React.memo(ChatInterfaceTabComponent);
