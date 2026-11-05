import React from 'react';
import { useTranslation } from 'react-i18next';
import { Switch } from '@/components/ui/switch';
import { IsolationLevel } from '@/lib/services/settings-service';
import type { AdvancedSystemSettingsProps } from './types';

// Behavior per level: src-tauri/src/session_isolation/platforms/ (unix.rs,
// linux.rs, macos.rs, windows.rs). Keep this text in sync with that code.
const ISOLATION_LEVEL_HELP: Record<IsolationLevel, string> = {
  basic:
    'Runs the command in the workspace with a cleared environment: only PATH and a few basic variables (such as HOME, USER, locale and proxy) are passed, so host secrets such as API keys are not. File and network access are not restricted.',
  medium:
    'Same as Basic, and the command runs in its own process group. File and network access are not restricted.',
  high: 'Linux: runs in separate user, PID and mount namespaces (unshare); file and network access are not restricted. macOS: runs under sandbox-exec; files can be written only in the workspace and /tmp, /var/tmp, and network access is allowed. Windows: same as Medium. If the sandbox is not available, Medium is used.',
};

interface AdvancedShellIsolationSectionProps {
  systemSettingsProps: AdvancedSystemSettingsProps;
}

function AdvancedShellIsolationSectionComponent({
  systemSettingsProps,
}: AdvancedShellIsolationSectionProps) {
  const { t } = useTranslation('common');
  const isolationLevel =
    systemSettingsProps.localSystemSettings.shellIsolationLevel;

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <div className="rounded-xl border border-border/70 p-4">
        <label
          htmlFor="shell-isolation-level"
          className="mb-2 block text-muted-foreground font-medium"
        >
          {t('settings.advanced.shellIsolation', 'Shell Isolation Level')}
        </label>
        <select
          id="shell-isolation-level"
          aria-describedby="shell-isolation-level-help"
          value={isolationLevel}
          onChange={(event) =>
            systemSettingsProps.onChange(
              'shellIsolationLevel',
              event.target.value as IsolationLevel,
            )
          }
          className="bg-background border text-foreground w-full max-w-xs rounded p-2"
        >
          <option value="basic">
            {t('settings.advanced.shellIsolationModes.basic', 'Basic')}
          </option>
          <option value="medium">
            {t('settings.advanced.shellIsolationModes.medium', 'Medium')}
          </option>
          <option value="high">
            {t(
              'settings.advanced.shellIsolationModes.high',
              'High - OS sandbox',
            )}
          </option>
        </select>
        <p
          id="shell-isolation-level-help"
          className="mt-2 text-xs text-foreground"
        >
          {t(
            `settings.advanced.shellIsolationHelp.${isolationLevel}`,
            ISOLATION_LEVEL_HELP[isolationLevel],
          )}
        </p>
        <p className="mt-1 text-xs text-muted-foreground">
          {t(
            'settings.advanced.shellIsolationDescription',
            'Applies to one-shot shell commands and background processes in local sessions. The persistent shell and Docker sessions do not use this setting.',
          )}
        </p>
      </div>

      <div className="rounded-xl border border-border/70 p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <label
              htmlFor="shell-runtime-bootstrap"
              className="block text-muted-foreground font-medium"
            >
              {t(
                'settings.advanced.shellRuntimeBootstrap',
                'Shell runtime bootstrap',
              )}
            </label>
            <p className="mt-1 text-xs text-muted-foreground">
              {t(
                'settings.advanced.shellRuntimeBootstrapDescription',
                'Loads only Python and Node paths in the terminal to start commands faster.',
              )}
            </p>
          </div>
          <Switch
            id="shell-runtime-bootstrap"
            checked={
              systemSettingsProps.localSystemSettings.shellRuntimeBootstrap ??
              false
            }
            onCheckedChange={(checked) =>
              systemSettingsProps.onChange('shellRuntimeBootstrap', checked)
            }
          />
        </div>
      </div>

      <div className="rounded-xl border border-border/70 p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <label
              htmlFor="allow-linked-worktrees"
              className="block text-muted-foreground font-medium"
            >
              {t(
                'settings.advanced.allowLinkedWorktrees',
                'File tools can use linked git worktrees',
              )}
            </label>
            <p className="mt-1 text-xs text-muted-foreground">
              {t(
                'settings.advanced.allowLinkedWorktreesDescription',
                'When the workspace is a git repository, file tools can also read and edit files in the other working folders of the same repository (git worktrees). Turn off to allow the workspace folder only.',
              )}
            </p>
          </div>
          <Switch
            id="allow-linked-worktrees"
            checked={
              systemSettingsProps.localSystemSettings.allowLinkedWorktrees ??
              true
            }
            onCheckedChange={(checked) =>
              systemSettingsProps.onChange('allowLinkedWorktrees', checked)
            }
          />
        </div>
      </div>
    </div>
  );
}

export const AdvancedShellIsolationSection = React.memo(
  AdvancedShellIsolationSectionComponent,
);
