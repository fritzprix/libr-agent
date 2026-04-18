import React from 'react';
import { useTranslation } from 'react-i18next';
import { AdvancedSettings, SystemSettings } from '@/context/SettingsContext';
import { IsolationLevel } from '@/lib/services/settings-service';
import { Input } from '@/components/ui';
import { DangerZoneSettings } from '../components/DangerZoneSettings';
import { AboutSection } from '../components/AboutSection';

interface AdvancedTabProps {
  localAdvancedSettings: AdvancedSettings;
  onChange: (key: keyof AdvancedSettings, value: number) => void;
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    networkSettingsChanged: boolean;
    onChange: (
      key: keyof SystemSettings,
      value: number | string | boolean,
    ) => void;
  };
  dangerZoneProps: {
    isDeleting: boolean;
    isResetting: boolean;
    onDelete: () => Promise<void>;
    onReset: () => Promise<void>;
  };
}

function AdvancedTabComponent({
  localAdvancedSettings,
  onChange,
  systemSettingsProps,
  dangerZoneProps,
}: AdvancedTabProps) {
  const { t } = useTranslation('common');

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-foreground">
          {t('settings.advanced.title', 'Advanced Runtime Controls')}
        </h3>
        <p className="mt-1 text-sm text-muted-foreground">
          {t(
            'settings.advanced.summary',
            'These settings change runtime safety rails, multi-agent limits, and shell isolation. Most users should leave them alone.',
          )}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 md:grid-cols-2">

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.loopPreventionThreshold',
              'Loop Prevention Threshold',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 3"
            min={2}
            max={20}
            step={1}
            value={localAdvancedSettings.loopPreventionThreshold ?? 3}
            onChange={(e) => {
              const parsed = parseInt(e.target.value, 10);
              const value = isNaN(parsed) ? 3 : Math.min(20, Math.max(2, parsed));
              onChange('loopPreventionThreshold', value);
            }}
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.loopPreventionThresholdDescription',
              'Number of identically repeated tool calls before the agent attempts natural recovery or triggers a hard stop.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.defaultSessionMaxDepth',
              'Session Branching Limit (Advanced)',
            )}
          </label>
          <Input
            type="number"
            placeholder="0 = unlimited"
            min={0}
            max={64}
            step={1}
            value={localAdvancedSettings.defaultSessionMaxDepth ?? 0}
            onChange={(e) =>
              onChange(
                'defaultSessionMaxDepth',
                parseInt(e.target.value, 10) || 0,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.defaultSessionMaxDepthDescription',
              'Controls how many child-session levels are allowed by default. Set 0 for unlimited. Most users can leave this as-is.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.defaultSessionMaxFanout',
              'Session Child Limit (Advanced)',
            )}
          </label>
          <Input
            type="number"
            placeholder="0 = unlimited"
            min={0}
            max={64}
            step={1}
            value={localAdvancedSettings.defaultSessionMaxFanout ?? 0}
            onChange={(e) =>
              onChange(
                'defaultSessionMaxFanout',
                parseInt(e.target.value, 10) || 0,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.defaultSessionMaxFanoutDescription',
              'Controls how many direct child sessions each parent can create by default. Set 0 for unlimited. Most users can leave this as-is.',
            )}
          </p>
        </div>
      </div>

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
        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.maxConcurrentActiveSessions',
              'Max Concurrent Agent Sessions',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 4"
            min={1}
            max={32}
            step={1}
            value={localAdvancedSettings.maxConcurrentActiveSessions ?? 4}
            onChange={(e) =>
              onChange(
                'maxConcurrentActiveSessions',
                parseInt(e.target.value, 10) || 4,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.maxConcurrentActiveSessionsDescription',
              'Maximum number of agent sessions running their LLM loop simultaneously. Higher values use more API quota and memory.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.maxSuspendedSessions',
              'Max Suspended Agent Sessions',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 8"
            min={1}
            max={64}
            step={1}
            value={localAdvancedSettings.maxSuspendedSessions ?? 8}
            onChange={(e) =>
              onChange(
                'maxSuspendedSessions',
                parseInt(e.target.value, 10) || 8,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.maxSuspendedSessionsDescription',
              'Maximum number of agent sessions that can be paused waiting for a child agent to complete. Should be ≥ active sessions.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.maxConcurrentActiveProcesses',
              'Max Concurrent Shell Processes',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 10"
            min={1}
            max={64}
            step={1}
            value={localAdvancedSettings.maxConcurrentActiveProcesses ?? 10}
            onChange={(e) =>
              onChange(
                'maxConcurrentActiveProcesses',
                parseInt(e.target.value, 10) || 10,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.maxConcurrentActiveProcessesDescription',
              'Maximum number of shell/code processes running simultaneously across all agent sessions.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.maxSuspendedProcesses',
              'Max Suspended Shell Processes',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 20"
            min={1}
            max={128}
            step={1}
            value={localAdvancedSettings.maxSuspendedProcesses ?? 20}
            onChange={(e) =>
              onChange(
                'maxSuspendedProcesses',
                parseInt(e.target.value, 10) || 20,
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.maxSuspendedProcessesDescription',
              'Maximum number of processes that can be paused waiting on pollProcess. Should be ≥ active processes.',
            )}
          </p>
        </div>

        <div className="min-w-0 rounded-xl border border-border/70 p-4">
          <label className="block text-muted-foreground mb-2 font-medium">
            {t(
              'settings.advanced.toolResultInlineLimit',
              'Tool Result Inline Limit (KB)',
            )}
          </label>
          <Input
            type="number"
            placeholder="e.g., 16"
            min={4}
            max={256}
            step={1}
            value={
              (localAdvancedSettings.toolResultInlineLimitBytes ?? 16 * 1024) /
              1024
            }
            onChange={(e) =>
              onChange(
                'toolResultInlineLimitBytes',
                Math.min(
                  256 * 1024,
                  Math.max(
                    4 * 1024,
                    (parseInt(e.target.value, 10) || 16) * 1024,
                  ),
                ),
              )
            }
            className="bg-background border text-foreground w-full max-w-xs"
          />
          <p className="text-xs text-muted-foreground mt-1">
            {t(
              'settings.advanced.toolResultInlineLimitDescription',
              'Controls how much tool output stays inline before LibrAgent spills the full result to a workspace file. Lower values keep the agent context leaner.',
            )}
          </p>
        </div>
      </div>

      <div className="min-w-0 rounded-xl border border-border/70 p-4">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.shellIsolation', 'Shell Isolation Level')}
        </label>
        <select
          value={systemSettingsProps.localSystemSettings.shellIsolationLevel}
          onChange={(e) =>
            systemSettingsProps.onChange(
              'shellIsolationLevel',
              e.target.value as IsolationLevel,
            )
          }
          className="bg-background border text-foreground w-full max-w-xs p-2 rounded"
        >
          <option value="basic">
            {t(
              'settings.advanced.shellIsolationModes.basic',
              'Basic - Full PATH access (less secure)',
            )}
          </option>
          <option value="medium">
            {t(
              'settings.advanced.shellIsolationModes.medium',
              'Medium - Restricted PATH (balanced)',
            )}
          </option>
          <option value="high">
            {t(
              'settings.advanced.shellIsolationModes.high',
              'High - Sandboxed (most secure)',
            )}
          </option>
        </select>
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.shellIsolationDescription',
            'Controls environment isolation for shell commands. Basic allows user-installed tools, High provides maximum security.',
          )}
        </p>
      </div>

      <AboutSection />
      <DangerZoneSettings {...dangerZoneProps} />
    </div>
  );
}

export default React.memo(AdvancedTabComponent, (prev, next) => {
  return (
    prev.localAdvancedSettings.loopPreventionThreshold ===
      next.localAdvancedSettings.loopPreventionThreshold &&
    prev.localAdvancedSettings.defaultSessionMaxDepth ===
      next.localAdvancedSettings.defaultSessionMaxDepth &&
    prev.localAdvancedSettings.defaultSessionMaxFanout ===
      next.localAdvancedSettings.defaultSessionMaxFanout &&
    prev.localAdvancedSettings.maxConcurrentActiveSessions ===
      next.localAdvancedSettings.maxConcurrentActiveSessions &&
    prev.localAdvancedSettings.maxSuspendedSessions ===
      next.localAdvancedSettings.maxSuspendedSessions &&
    prev.localAdvancedSettings.maxConcurrentActiveProcesses ===
      next.localAdvancedSettings.maxConcurrentActiveProcesses &&
    prev.localAdvancedSettings.maxSuspendedProcesses ===
      next.localAdvancedSettings.maxSuspendedProcesses &&
    prev.localAdvancedSettings.toolResultInlineLimitBytes ===
      next.localAdvancedSettings.toolResultInlineLimitBytes &&
    prev.onChange === next.onChange &&
    prev.systemSettingsProps.localSystemSettings ===
      next.systemSettingsProps.localSystemSettings &&
    prev.systemSettingsProps.onChange === next.systemSettingsProps.onChange &&
    prev.dangerZoneProps.isDeleting === next.dangerZoneProps.isDeleting &&
    prev.dangerZoneProps.isResetting === next.dangerZoneProps.isResetting
  );
});
