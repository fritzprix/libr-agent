const PLUGIN_INSTALL_RE = /^\/plugin\s+install\s+(\S+)$/i;

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
