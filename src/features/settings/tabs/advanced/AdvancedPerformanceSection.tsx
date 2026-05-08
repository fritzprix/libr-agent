import React from 'react';
import { useTranslation } from 'react-i18next';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import {
  formatBytesAsKilobytes,
  parseIntegerInput,
  parseKilobytesInputToBytes,
} from '@/features/settings/components/settings-number-utils';
import type { AdvancedSettingsSectionProps } from './types';

function AdvancedPerformanceSectionComponent({
  localAdvancedSettings,
  onChange,
}: AdvancedSettingsSectionProps) {
  const { t } = useTranslation('common');

  return (
    <>
      <div>
        <h4 className="text-sm font-medium text-foreground">
          {t('settings.advanced.performance', 'Performance & Concurrency')}
        </h4>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.advanced.performanceDescription',
            'These runtime limits control how many agents and workspace processes can run at once.',
          )}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        <NumberSettingField
          label={t(
            'settings.advanced.maxConcurrentActiveSessions',
            'Max Concurrent Agent Sessions',
          )}
          description={t(
            'settings.advanced.maxConcurrentActiveSessionsDescription',
            'Maximum number of agent sessions running their LLM loop simultaneously. Higher values use more API quota and memory.',
          )}
          placeholder={t(
            'settings.advanced.maxConcurrentActiveSessionsPlaceholder',
            'e.g., 4',
          )}
          min={1}
          max={32}
          step={1}
          value={localAdvancedSettings.maxConcurrentActiveSessions ?? 4}
          parseValue={(rawValue) =>
            parseIntegerInput(rawValue, {
              fallback: 4,
              min: 1,
              max: 32,
            })
          }
          onValueChange={(value) =>
            onChange('maxConcurrentActiveSessions', value)
          }
        />

        <NumberSettingField
          label={t(
            'settings.advanced.maxSuspendedSessions',
            'Max Suspended Agent Sessions',
          )}
          description={t(
            'settings.advanced.maxSuspendedSessionsDescription',
            'Maximum number of agent sessions that can be paused waiting for a child agent to complete. Should be ≥ active sessions.',
          )}
          placeholder={t(
            'settings.advanced.maxSuspendedSessionsPlaceholder',
            'e.g., 8',
          )}
          min={1}
          max={64}
          step={1}
          value={localAdvancedSettings.maxSuspendedSessions ?? 8}
          parseValue={(rawValue) =>
            parseIntegerInput(rawValue, {
              fallback: 8,
              min: 1,
              max: 64,
            })
          }
          onValueChange={(value) => onChange('maxSuspendedSessions', value)}
        />

        <NumberSettingField
          label={t(
            'settings.advanced.maxConcurrentActiveProcesses',
            'Max Concurrent Shell Processes',
          )}
          description={t(
            'settings.advanced.maxConcurrentActiveProcessesDescription',
            'Maximum number of shell/code processes running simultaneously across all agent sessions.',
          )}
          placeholder={t(
            'settings.advanced.maxConcurrentActiveProcessesPlaceholder',
            'e.g., 10',
          )}
          min={1}
          max={64}
          step={1}
          value={localAdvancedSettings.maxConcurrentActiveProcesses ?? 10}
          parseValue={(rawValue) =>
            parseIntegerInput(rawValue, {
              fallback: 10,
              min: 1,
              max: 64,
            })
          }
          onValueChange={(value) =>
            onChange('maxConcurrentActiveProcesses', value)
          }
        />

        <NumberSettingField
          label={t(
            'settings.advanced.maxSuspendedProcesses',
            'Max Suspended Shell Processes',
          )}
          description={t(
            'settings.advanced.maxSuspendedProcessesDescription',
            'Maximum number of processes that can be paused waiting on pollProcess. Should be ≥ active processes.',
          )}
          placeholder={t(
            'settings.advanced.maxSuspendedProcessesPlaceholder',
            'e.g., 20',
          )}
          min={1}
          max={128}
          step={1}
          value={localAdvancedSettings.maxSuspendedProcesses ?? 20}
          parseValue={(rawValue) =>
            parseIntegerInput(rawValue, {
              fallback: 20,
              min: 1,
              max: 128,
            })
          }
          onValueChange={(value) => onChange('maxSuspendedProcesses', value)}
        />

        <NumberSettingField
          label={t(
            'settings.advanced.toolResultInlineLimit',
            'Tool Result Inline Limit (KB)',
          )}
          description={t(
            'settings.advanced.toolResultInlineLimitDescription',
            'Controls how much tool output stays inline before LibrAgent spills the full result to a workspace file. Lower values keep the agent context leaner.',
          )}
          placeholder={t(
            'settings.advanced.toolResultInlineLimitPlaceholder',
            'e.g., 16',
          )}
          min={4}
          max={256}
          step={1}
          value={formatBytesAsKilobytes(
            localAdvancedSettings.toolResultInlineLimitBytes,
            16 * 1024,
          )}
          parseValue={(rawValue) =>
            parseKilobytesInputToBytes(rawValue, {
              fallbackKilobytes: 16,
              minKilobytes: 4,
              maxKilobytes: 256,
            })
          }
          onValueChange={(value) =>
            onChange('toolResultInlineLimitBytes', value)
          }
        />
      </div>
    </>
  );
}

export const AdvancedPerformanceSection = React.memo(
  AdvancedPerformanceSectionComponent,
);
