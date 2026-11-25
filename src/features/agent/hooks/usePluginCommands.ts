import { useCallback, useEffect, useState } from 'react';
import { safeInvoke } from '@/lib/backend/core';
import { getLogger } from '@/lib/logger';
import type { PluginSlashCommand } from '../lib/plugin-command';

const logger = getLogger('usePluginCommands');

/**
 * Plugin slash commands available to a session (#2922).
 *
 * Loaded on session change and refreshed explicitly after `/plugin
 * uninstall` or `/reload` (D5: enablement changes apply at session open
 * or reload).
 */
export function usePluginCommands(sessionId: string | undefined) {
  const [commands, setCommands] = useState<PluginSlashCommand[]>([]);

  const refresh = useCallback(async () => {
    if (!sessionId) {
      setCommands([]);
      return;
    }
    try {
      const list = await safeInvoke<PluginSlashCommand[]>(
        'agent_list_plugin_commands',
        { sessionId },
      );
      setCommands(list ?? []);
    } catch (err) {
      logger.warn('Failed to list plugin slash commands', err);
      setCommands([]);
    }
  }, [sessionId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return { commands, refresh };
}
