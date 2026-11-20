const PLUGIN_INSTALL_RE = /^\/plugin\s+install\s+(\S+)$/i;

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
 * Extracts the source URL from a `/plugin install <git-url>` command.
 *
 * Install needs the hook-approval dialog, so it is handled by the desktop
 * input instead of the generic `agent_execute_command` path. Uninstall and
 * list do not need approval and stay on the backend command path.
 */
export function parsePluginInstallCommand(commandText: string): string | null {
  const match = PLUGIN_INSTALL_RE.exec(commandText.trim());
  return match ? match[1] : null;
}
