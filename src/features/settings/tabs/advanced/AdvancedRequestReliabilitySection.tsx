import React from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui';
import type { AdvancedSettingsSectionProps } from './types';

const RETRY_DELAY_PRESETS = [1000, 3000, 5000, 10000] as const;

function AdvancedRequestReliabilitySectionComponent({
  localAdvancedSettings,
  onChange,
}: AdvancedSettingsSectionProps) {
  const { t } = useTranslation('common');
  const maxRetries = localAdvancedSettings.maxRetries;
  const retryDelay = localAdvancedSettings.retryDelay;

  return (
    <>
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.advanced.requestReliability', 'Request Reliability')}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.advanced.requestReliabilityDescription',
            'Controls how failed AI provider requests are retried.',
          )}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        <div className="min-w-0">
          <label className="mb-2 block text-muted-foreground font-medium">
            {t('settings.advanced.maxRetries', 'Max Retry Attempts')}
          </label>
          <div className="flex max-w-xs items-center gap-2">
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() =>
                onChange('maxRetries', Math.max(0, maxRetries - 1))
              }
              disabled={maxRetries <= 0}
              aria-label={t(
                'settings.aiModels.decreaseRetries',
                'Decrease retry attempts',
              )}
            >
              -
            </Button>
            <div className="flex h-9 min-w-[4rem] items-center justify-center rounded-md border bg-background px-3 text-sm font-medium">
              {maxRetries}
            </div>
            <Button
              type="button"
              variant="outline"
              className="h-9 w-9 px-0"
              onClick={() =>
                onChange('maxRetries', Math.min(5, maxRetries + 1))
              }
              disabled={maxRetries >= 5}
              aria-label={t(
                'settings.aiModels.increaseRetries',
                'Increase retry attempts',
              )}
            >
              +
            </Button>
          </div>
          <p className="mt-3 text-xs text-muted-foreground">
            {t(
              'settings.advanced.maxRetriesDescription',
              'Maximum number of retries for failed AI requests.',
            )}
          </p>
        </div>

        <div className="min-w-0">
          <label className="mb-2 block text-muted-foreground font-medium">
            {t('settings.advanced.retryDelay', 'Retry Delay (ms)')}
          </label>
          <div className="flex flex-wrap gap-2">
            {RETRY_DELAY_PRESETS.map((preset) => (
              <Button
                key={preset}
                type="button"
                variant={preset === retryDelay ? 'default' : 'outline'}
                className="h-8 px-3 text-xs"
                onClick={() => onChange('retryDelay', preset)}
              >
                {`${preset / 1000}s`}
              </Button>
            ))}
          </div>
          <p className="mt-3 text-xs text-muted-foreground">
            {t(
              'settings.advanced.retryDelayDescription',
              'Delay in milliseconds between retry attempts.',
            )}
          </p>
        </div>
      </div>
    </>
  );
}

export const AdvancedRequestReliabilitySection = React.memo(
  AdvancedRequestReliabilitySectionComponent,
);
