import { useTranslation } from 'react-i18next';
import { SystemSettings } from '@/context/SettingsContext';
import { Input } from '@/components/ui';
import { Badge } from '@/components/ui';

interface SystemPerformanceSettingsProps {
  localSystemSettings: SystemSettings;
  networkSettingsChanged: boolean;
  onChange: (key: keyof SystemSettings, value: number | boolean) => void;
}

export function SystemPerformanceSettings({
  localSystemSettings,
  networkSettingsChanged,
  onChange,
}: SystemPerformanceSettingsProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-8">
      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
        <div className="space-y-4 rounded-xl border border-border/70 p-4">
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.system.backgroundTasks', 'Background Tasks')}
          </h4>
          {/* Search Index Frequency */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.searchIndexFrequency',
                'Search Index Frequency (Min)',
              )}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.searchIndexFrequency',
                'e.g., 5',
              )}
              min={1}
              value={localSystemSettings.searchIndexFrequencyMinutes}
              onChange={(e) =>
                onChange(
                  'searchIndexFrequencyMinutes',
                  parseInt(e.target.value, 10) || 5,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.searchIndexFrequencyDescription',
                'How often the AI updates its memory search. Faster updates keep search fresh but use more battery/CPU.',
              )}
            </p>
          </div>

          {/* Web Action Timeout */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.webActionTimeout',
                'Web Action Timeout (Sec)',
              )}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.webActionTimeout',
                'e.g., 30',
              )}
              min={5}
              value={localSystemSettings.webActionTimeoutSeconds}
              onChange={(e) =>
                onChange(
                  'webActionTimeoutSeconds',
                  parseInt(e.target.value, 10) || 30,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.webActionTimeoutDescription',
                'How long the AI waits for a webpage to load or a click to finish.',
              )}
            </p>
          </div>

          {/* MCP Server Startup Timeout */}
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.mcpServerStartupTimeout',
                'MCP Server Startup Timeout (Sec)',
              )}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.mcpServerStartupTimeout',
                'e.g., 60',
              )}
              min={10}
              max={120}
              value={localSystemSettings.mcpServerStartupTimeoutSeconds}
              onChange={(e) =>
                onChange(
                  'mcpServerStartupTimeoutSeconds',
                  parseInt(e.target.value, 10) || 60,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.mcpServerStartupTimeoutDescription',
                'How long to wait for MCP tool servers to initialize. Increase if servers fail to start.',
              )}
            </p>
          </div>

          {/* MCP Tool Execution Timeout */}
          <div className="min-w-0">
            <div className="flex items-center gap-2 mb-2">
              <label className="block text-muted-foreground font-medium">
                {t(
                  'settings.system.mcpToolTimeout',
                  'MCP Tool Execution Timeout (Sec)',
                )}
              </label>
              {(localSystemSettings.mcpToolTimeoutSeconds ?? 0) === 0 && (
                <Badge variant="outline" className="text-xs">
                  {t('settings.system.mcpToolTimeoutDisabled', 'Disabled')}
                </Badge>
              )}
            </div>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.mcpToolTimeout',
                '0 (disabled)',
              )}
              min={0}
              value={localSystemSettings.mcpToolTimeoutSeconds ?? 0}
              onChange={(e) =>
                onChange(
                  'mcpToolTimeoutSeconds',
                  parseInt(e.target.value, 10) || 0,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.mcpToolTimeoutDescription',
                'How long to wait for a single tool call before cancelling it. Set to 0 to disable (recommended for long-running agent tools like awaitAgent).',
              )}
            </p>
          </div>
        </div>

        <div className="space-y-4 rounded-xl border border-border/70 p-4">
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.system.automation', 'Automation Governance')}
          </h4>
          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.scheduledTaskMinimumInterval',
                'Scheduled Task Minimum Interval (Min)',
              )}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.scheduledTaskMinimumInterval',
                '0 = disabled',
              )}
              min={0}
              value={localSystemSettings.scheduledTaskMinimumIntervalMinutes}
              onChange={(e) =>
                onChange(
                  'scheduledTaskMinimumIntervalMinutes',
                  parseInt(e.target.value, 10) || 0,
                )
              }
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.scheduledTaskMinimumIntervalDescription',
                'Minimum allowed interval for new or re-enabled scheduled tasks. Set 0 to disable the guard.',
              )}
            </p>
          </div>

          <div className="min-w-0">
            <label className="block text-muted-foreground mb-2 font-medium">
              {t(
                'settings.system.maxScheduledTaskGroups',
                'Max Scheduled Task Groups',
              )}
            </label>
            <Input
              type="number"
              placeholder={t(
                'settings.system.placeholders.maxScheduledTaskGroups',
                'e.g., 10',
              )}
              min={1}
              max={100}
              value={localSystemSettings.maxScheduledTaskGroups}
              onChange={(e) => {
                const parsedValue = Number.parseInt(e.target.value, 10);
                const nextValue = Number.isNaN(parsedValue)
                  ? 10
                  : Math.min(100, Math.max(1, parsedValue));

                onChange('maxScheduledTaskGroups', nextValue);
              }}
              className="bg-background border text-foreground w-full max-w-xs"
            />
            <p className="text-xs text-muted-foreground mt-1">
              {t(
                'settings.system.maxScheduledTaskGroupsDescription',
                'Maximum number of distinct scheduled task groups allowed across recurring teamwork automation.',
              )}
            </p>
          </div>
        </div>
      </div>

      <div className="space-y-4 rounded-xl border border-border/70 p-4">
        <div className="flex items-center gap-2">
          <h4 className="text-sm font-medium text-foreground">
            {t('settings.system.network', 'Network')}
          </h4>
          {networkSettingsChanged && (
            <Badge
              variant="outline"
              className="border-warning/30 bg-warning/10 text-warning-foreground"
            >
              {t('settings.system.restartRequired', 'Restart required')}
            </Badge>
          )}
        </div>
        <div
          className={
            networkSettingsChanged
              ? 'rounded-lg border border-warning/30 bg-warning/10 px-3 py-2'
              : ''
          }
        >
          <p
            className={
              networkSettingsChanged
                ? 'text-xs text-warning-foreground'
                : 'text-xs text-muted-foreground'
            }
          >
            {networkSettingsChanged
              ? t(
                  'settings.system.networkRestartPending',
                  'You changed network settings. Save changes, then restart the app to apply them.',
                )
              : t(
                  'settings.system.networkRestartNotice',
                  'Changes to HTTP server network settings are applied after restarting the app.',
                )}
          </p>
        </div>

        {/* HTTP Server Port */}
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.system.httpServerPort', 'HTTP Server Port')}
          </label>
          <Input
            type="number"
            placeholder={t(
              'settings.system.placeholders.httpServerPort',
              'e.g., 3030',
            )}
            min={1}
            max={65535}
            value={localSystemSettings.httpServerPort ?? 3030}
            onChange={(e) =>
              onChange('httpServerPort', parseInt(e.target.value, 10) || 3030)
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.system.httpServerPortDescription',
              'Port used by the internal HTTP API server.',
            )}
          </p>
        </div>

        {/* HTTP Server Exposure */}
        <div className="min-w-0">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t('settings.system.httpServerExpose', 'Expose HTTP Server')}
          </label>
          <select
            value={localSystemSettings.httpServerExpose ? 'public' : 'local'}
            onChange={(e) =>
              onChange('httpServerExpose', e.target.value === 'public')
            }
            className="bg-background border text-foreground w-full max-w-xs p-2 rounded"
          >
            <option value="local">
              {t(
                'settings.system.httpServerExposeMode.local',
                'Local only (127.0.0.1)',
              )}
            </option>
            <option value="public">
              {t(
                'settings.system.httpServerExposeMode.public',
                'Expose to network (0.0.0.0)',
              )}
            </option>
          </select>
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.system.httpServerExposeDescription',
              'Use local-only by default. Exposing allows access from other devices on your network.',
            )}
          </p>
          {localSystemSettings.httpServerExpose && (
            <p className="text-xs text-warning mt-2">
              {t(
                'settings.system.httpServerExposeWarning',
                'Warning: HTTP API is exposed to your network (0.0.0.0). Use only in trusted networks and protect access appropriately.',
              )}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
