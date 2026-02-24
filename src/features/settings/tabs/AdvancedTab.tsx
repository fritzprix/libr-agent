import React from 'react';
import { useTranslation } from 'react-i18next';
import { AdvancedSettings, SystemSettings } from '@/context/SettingsContext';
import { IsolationLevel } from '@/lib/services/settings-service';
import { Input } from '@/components/ui';
import { SystemPerformanceSettings } from '../components/SystemPerformanceSettings';
import { DangerZoneSettings } from '../components/DangerZoneSettings';
import { AboutSection } from '../components/AboutSection';

interface AdvancedTabProps {
  localAdvancedSettings: AdvancedSettings;
  onChange: (key: keyof AdvancedSettings, value: number) => void;
  systemSettingsProps: {
    localSystemSettings: SystemSettings;
    onChange: (
      key: keyof SystemSettings,
      value: number | string | boolean,
    ) => void;
  };
  dangerZoneProps: {
    isDeleting: boolean;
    isResetting: boolean;
    onDelete: () => Promise<void>;
    onReset: () => void;
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
      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.maxRetries', 'Max Retry Attempts')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 1"
          min={0}
          max={5}
          value={localAdvancedSettings.maxRetries}
          onChange={(e) =>
            onChange('maxRetries', parseInt(e.target.value, 10) || 0)
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.maxRetriesDescription',
            'Maximum number of retries for failed AI requests.',
          )}
        </p>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.retryDelay', 'Retry Delay (ms)')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 5000"
          min={1000}
          step={1000}
          value={localAdvancedSettings.retryDelay}
          onChange={(e) =>
            onChange('retryDelay', parseInt(e.target.value, 10) || 5000)
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.retryDelayDescription',
            'Delay in milliseconds between retry attempts.',
          )}
        </p>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t('settings.advanced.circuitBreaker', 'Tool Loop Threshold')}
        </label>
        <Input
          type="number"
          placeholder="e.g., 3"
          min={1}
          max={10}
          value={localAdvancedSettings.circuitBreakerThreshold}
          onChange={(e) =>
            onChange(
              'circuitBreakerThreshold',
              parseInt(e.target.value, 10) || 3,
            )
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.circuitBreakerDescription',
            'Number of repeated errors or tool calls before triggering the circuit breaker.',
          )}
        </p>
      </div>

      <div className="min-w-0">
        <label className="block text-muted-foreground mb-2 font-medium">
          {t(
            'settings.advanced.maxOutputTokens',
            'Max Output Tokens (Default)',
          )}
        </label>
        <Input
          type="number"
          placeholder="e.g., 8192"
          min={256}
          max={128000}
          step={256}
          value={localAdvancedSettings.defaultMaxOutputTokens ?? 8192} // Fallback for transition
          onChange={(e) =>
            onChange(
              'defaultMaxOutputTokens',
              parseInt(e.target.value, 10) || 8192,
            )
          }
          className="bg-background border text-foreground w-full max-w-xs"
        />
        <p className="text-xs text-muted-foreground mt-1">
          {t(
            'settings.advanced.maxOutputTokensDescription',
            'Default maximum output tokens for new sessions if not specified by assistant.',
          )}
        </p>
      </div>

      <div className="min-w-0">
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

      <div className="min-w-0">
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

      {/* SP2: Global Concurrency Control */}
      <div className="min-w-0">
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

      <div className="min-w-0">
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
            onChange('maxSuspendedSessions', parseInt(e.target.value, 10) || 8)
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

      <div className="min-w-0">
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

      <div className="min-w-0">
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

      <div className="min-w-0">
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

      <SystemPerformanceSettings {...systemSettingsProps} />
      <AboutSection />
      <DangerZoneSettings {...dangerZoneProps} />
    </div>
  );
}

export default React.memo(AdvancedTabComponent, (prev, next) => {
  return (
    prev.localAdvancedSettings.maxRetries ===
      next.localAdvancedSettings.maxRetries &&
    prev.localAdvancedSettings.retryDelay ===
      next.localAdvancedSettings.retryDelay &&
    prev.localAdvancedSettings.circuitBreakerThreshold ===
      next.localAdvancedSettings.circuitBreakerThreshold &&
    prev.localAdvancedSettings.defaultMaxOutputTokens ===
      next.localAdvancedSettings.defaultMaxOutputTokens &&
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
    prev.onChange === next.onChange &&
    prev.systemSettingsProps.localSystemSettings ===
      next.systemSettingsProps.localSystemSettings &&
    prev.systemSettingsProps.onChange === next.systemSettingsProps.onChange &&
    prev.dangerZoneProps.isDeleting === next.dangerZoneProps.isDeleting &&
    prev.dangerZoneProps.isResetting === next.dangerZoneProps.isResetting
  );
});
