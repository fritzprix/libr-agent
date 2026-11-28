const PLUGIN_INSTALL_RE = /^\/plugin\s+install\s+(?:"([^"]+)"|(\S+))$/i;
const PLUGIN_UPDATE_RE = /^\/plugin\s+update\s+(\S+)$/i;

/**
 * A plugin slash command offered by the chat input (#2922). Mirrors
 * `PluginCommandInfo` in `src-tauri/src/services/skill_service/plugin_commands.rs`.
 */
export interface PluginSlashCommand {
  /** Exact text to type, e.g. `/hello` or `/superpowers:hello`. */
  label: string;
  name: string;
  pluginName: string;
  description: string;
  argumentHint?: string;
  scope: 'global' | 'session';
}

/** Command dropdown entry built from a plugin command. */
export function pluginCommandToEntry(command: PluginSlashCommand): {
  id: string;
  label: string;
  description: string;
} {
  const description = [command.description, command.argumentHint]
    .filter((part) => !!part)
    .join(' ');
  return { id: command.label, label: command.label, description };
}

/**
 * Extracts the install source from a `/plugin install <git-url|path>`
 * command (#2928: github.com URL, any git URL, or a local folder; a path
 * may be wrapped in double quotes to contain spaces).
 *
 * Install needs the hook-approval dialog, so it is handled by the desktop
 * input instead of the generic `agent_execute_command` path. Uninstall and
 * list do not need approval and stay on the backend command path.
 */
export function parsePluginInstallCommand(commandText: string): string | null {
  const match = PLUGIN_INSTALL_RE.exec(commandText.trim());
  if (!match) {
    return null;
  }
  return match[1] ?? match[2] ?? null;
}

/**
 * Extracts the target from a `/plugin update <name|git-url>` command
 * (#2927 item 4). Update re-downloads an installed plugin and needs the D6
 * notice plus the install-time approvals again, so — like install — it is
 * handled by the desktop input, not the generic `agent_execute_command`
 * path.
 */
export function parsePluginUpdateCommand(commandText: string): string | null {
  const match = PLUGIN_UPDATE_RE.exec(commandText.trim());
  return match ? match[1] : null;
}
