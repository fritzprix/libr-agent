import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui';
import type { AdvancedSettingsSectionProps } from './types';

const THINKING_BUDGET_PRESETS = [
  { label: 'Off', value: 0 },
  { label: 'Low', value: 1024 },
  { label: 'Medium', value: 8192 },
  { label: 'High', value: 24576 },
  { label: 'Dynamic', value: -1 },
] as const;

function AdvancedThinkingBudgetSectionComponent({
  localAdvancedSettings,
  onChange,
}: AdvancedSettingsSectionProps) {
  const { t } = useTranslation('common');
  const budget = localAdvancedSettings.thinkingBudget;

  return (
    <>
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.advanced.thinkingBudget', 'Thinking / Reasoning Budget')}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.advanced.thinkingBudgetDescription',
            'Controls how much the model can "think" before answering. Higher budgets produce deeper reasoning at higher cost. Models that don\'t support thinking will ignore this setting.',
          )}
        </p>
      </div>

      <div className="flex flex-wrap gap-2">
        {THINKING_BUDGET_PRESETS.map((preset) => (
          <Button
            key={preset.value}
            type="button"
            variant={preset.value === budget ? 'default' : 'outline'}
            className="h-8 px-3 text-xs"
            onClick={() => onChange('thinkingBudget', preset.value)}
          >
            {preset.label}
            {preset.value > 0 && (
              <span className="ml-1 text-muted-foreground">
                (
                {preset.value >= 1024
                  ? `${preset.value / 1024}K`
                  : preset.value}
                )
              </span>
            )}
          </Button>
        ))}
      </div>
      <p className="mt-3 text-xs text-muted-foreground">
        {budget === 0 &&
          t('settings.advanced.thinkingBudgetOff', 'Thinking is disabled.')}
        {budget === -1 &&
          t(
            'settings.advanced.thinkingBudgetDynamic',
            'Model auto-adjusts thinking budget.',
          )}
        {budget === 1024 &&
          t(
            'settings.advanced.thinkingBudgetLow',
            '~1K tokens — fast, minimal reasoning.',
          )}
        {budget === 8192 &&
          t(
            'settings.advanced.thinkingBudgetMedium',
            '~8K tokens — balanced reasoning (recommended).',
          )}
        {budget === 24576 &&
          t(
            'settings.advanced.thinkingBudgetHigh',
            '~24K tokens — deep reasoning, higher cost.',
          )}
      </p>
    </>
  );
}

export const AdvancedThinkingBudgetSection = React.memo(
  AdvancedThinkingBudgetSectionComponent,
);
