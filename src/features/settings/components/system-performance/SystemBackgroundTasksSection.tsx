import React from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui';
import { NumberSettingField } from '@/features/settings/components/NumberSettingField';
import { parseIntegerInput } from '@/features/settings/components/settings-number-utils';
import type { SystemPerformanceSettingsProps } from './types';

type SystemBackgroundTasksSectionProps = Pick<
  SystemPerformanceSettingsProps,
  'localSystemSettings' | 'onChange'
>;

function SystemBackgroundTasksSectionComponent({
  localSystemSettings,
  onChange,
}: SystemBackgroundTasksSectionProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-4 rounded-xl border border-border/70 p-4">
      <h4 className="text-sm font-medium text-foreground">
        {t('settings.system.backgroundTasks', 'Background Tasks')}
      </h4>

      <NumberSettingField
        label={t(
          'settings.system.searchIndexFrequency',
          'Search Index Frequency (Min)',
        )}
        description={t(
          'settings.system.searchIndexFrequencyDescription',
          'How often the AI updates its memory search. Faster updates keep search fresh but use more battery/CPU.',
        )}
        placeholder={t(
          'settings.system.placeholders.searchIndexFrequency',
          'e.g., 5',
        )}
        min={1}
        value={localSystemSettings.searchIndexFrequencyMinutes}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 5,
            min: 1,
          })
        }
        onValueChange={(value) =>
          onChange('searchIndexFrequencyMinutes', value)
        }
        containerClassName="min-w-0"
      />

      <NumberSettingField
        label={t(
          'settings.system.webActionTimeout',
          'Web Action Timeout (Sec)',
        )}
        description={t(
          'settings.system.webActionTimeoutDescription',
          'How long the AI waits for a webpage to load or a click to finish.',
        )}
        placeholder={t(
          'settings.system.placeholders.webActionTimeout',
          'e.g., 30',
        )}
        min={5}
        value={localSystemSettings.webActionTimeoutSeconds}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 30,
            min: 5,
          })
        }
        onValueChange={(value) => onChange('webActionTimeoutSeconds', value)}
        containerClassName="min-w-0"
      />

      <NumberSettingField
        label={t(
          'settings.system.mcpServerStartupTimeout',
          'MCP Server Startup Timeout (Sec)',
        )}
        description={t(
          'settings.system.mcpServerStartupTimeoutDescription',
          'How long to wait for MCP tool servers to initialize. Increase if servers fail to start.',
        )}
        placeholder={t(
          'settings.system.placeholders.mcpServerStartupTimeout',
          'e.g., 60',
        )}
        min={10}
        max={120}
        value={localSystemSettings.mcpServerStartupTimeoutSeconds}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 60,
            min: 10,
            max: 120,
          })
        }
        onValueChange={(value) =>
          onChange('mcpServerStartupTimeoutSeconds', value)
        }
        containerClassName="min-w-0"
      />

      <NumberSettingField
        label={t(
          'settings.system.mcpServerVerificationTimeout',
          'MCP Server Verification Timeout (Sec)',
        )}
        description={t(
          'settings.system.mcpServerVerificationTimeoutDescription',
          'How long to wait for MCP server verification. Prevents indefinite UI lock when testing connections.',
        )}
        placeholder={t(
          'settings.system.placeholders.mcpServerVerificationTimeout',
          'e.g., 30',
        )}
        min={5}
        max={120}
        value={localSystemSettings.mcpServerVerificationTimeoutSeconds}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 30,
            min: 5,
            max: 120,
          })
        }
        onValueChange={(value) =>
          onChange('mcpServerVerificationTimeoutSeconds', value)
        }
        containerClassName="min-w-0"
      />

      <NumberSettingField
        label={t(
          'settings.system.mcpToolTimeout',
          'MCP Tool Execution Timeout (Sec)',
        )}
        labelAdornment={
          (localSystemSettings.mcpToolTimeoutSeconds ?? 0) === 0 ? (
            <Badge variant="outline" className="text-xs">
              {t('settings.system.mcpToolTimeoutDisabled', 'Disabled')}
            </Badge>
          ) : undefined
        }
        description={t(
          'settings.system.mcpToolTimeoutDescription',
          'How long to wait for a single tool call before cancelling it. Set to 0 to disable (recommended for long-running agent tools like awaitAgent).',
        )}
        placeholder={t(
          'settings.system.placeholders.mcpToolTimeout',
          '0 (disabled)',
        )}
        min={0}
        value={localSystemSettings.mcpToolTimeoutSeconds ?? 0}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 0,
            min: 0,
          })
        }
        onValueChange={(value) => onChange('mcpToolTimeoutSeconds', value)}
        containerClassName="min-w-0"
      />
    </div>
  );
}

export const SystemBackgroundTasksSection = React.memo(
  SystemBackgroundTasksSectionComponent,
);
