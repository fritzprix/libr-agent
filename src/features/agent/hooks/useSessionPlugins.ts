import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { listPluginCommands, listSessionPlugins } from '@/lib/backend/skills';
import type { PluginCommandInfo, SessionPluginsOverview } from '@/types/skills';

const logger = getLogger('useSessionPlugins');

const SESSION_PLUGINS_KEY = 'session-plugins';
const PLUGIN_COMMANDS_KEY = 'plugin-commands';

/** SWR key filter matching every `useSessionPlugins` cache entry. */
export function isSessionPluginsKey(key: unknown): boolean {
  return Array.isArray(key) && key[0] === SESSION_PLUGINS_KEY;
}

/** SWR key filter matching every `usePluginCommands` cache entry (#2922). */
export function isPluginCommandsKey(key: unknown): boolean {
  return Array.isArray(key) && key[0] === PLUGIN_COMMANDS_KEY;
}

/** Active (global + session) plugins and the install history of a session. */
export function useSessionPlugins(sessionId: string | undefined) {
  const { data, isLoading, mutate } = useSWR(
    sessionId ? ([SESSION_PLUGINS_KEY, sessionId] as const) : null,
    ([, id]): Promise<SessionPluginsOverview> => listSessionPlugins(id),
    {
      onError: (error) => logger.warn('Failed to load session plugins', error),
    },
  );

  return { overview: data, isLoading, refresh: mutate };
}

/** Slash commands contributed by plugins active in a session (#2922). */
export function usePluginCommands(sessionId: string | undefined) {
  const { data, mutate } = useSWR(
    sessionId ? ([PLUGIN_COMMANDS_KEY, sessionId] as const) : null,
    ([, id]): Promise<PluginCommandInfo[]> => listPluginCommands(id),
    {
      onError: (error) => logger.warn('Failed to load plugin commands', error),
    },
  );

  return { commands: data ?? [], refresh: mutate };
}
