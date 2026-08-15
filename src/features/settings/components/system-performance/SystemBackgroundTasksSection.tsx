import React from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui';
import { Switch } from '@/components/ui/switch';
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

      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <label
            htmlFor="prevent-sleep-during-agent-work"
            className="block text-muted-foreground font-medium"
          >
            {t(
              'settings.system.preventSleepDuringAgentWork',
              'Prevent sleep while app is running',
            )}
          </label>
          <p className="mt-1 text-xs text-muted-foreground">
            {t(
              'settings.system.preventSleepDuringAgentWorkDescription',
              'Keep the system from idle-sleeping while LibrAgent is open. Does not force the display to stay on. Enabled by default.',
            )}
          </p>
        </div>
        <Switch
          id="prevent-sleep-during-agent-work"
          checked={localSystemSettings.preventSleepDuringAgentWork ?? true}
          onCheckedChange={(checked) =>
            onChange('preventSleepDuringAgentWork', checked)
          }
        />
      </div>

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
          'MCP Discovery Timeout (Sec)',
        )}
        description={t(
          'settings.system.mcpServerStartupTimeoutDescription',
          'How long to wait for MCP servers to finish tool discovery before marking them timed out and opening the session. Increase for slow servers (e.g. npx first run).',
        )}
        placeholder={t(
          'settings.system.placeholders.mcpServerStartupTimeout',
          'e.g., 30',
        )}
        min={10}
        max={120}
        value={localSystemSettings.mcpServerStartupTimeoutSeconds}
        parseValue={(rawValue) =>
          parseIntegerInput(rawValue, {
            fallback: 30,
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
