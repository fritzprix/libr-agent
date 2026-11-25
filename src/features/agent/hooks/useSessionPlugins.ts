import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { listSessionPlugins } from '@/lib/backend/skills';
import type { SessionPluginsOverview } from '@/types/skills';

const logger = getLogger('useSessionPlugins');

const SESSION_PLUGINS_KEY = 'session-plugins';

/** SWR key filter matching every `useSessionPlugins` cache entry. */
export function isSessionPluginsKey(key: unknown): boolean {
  return Array.isArray(key) && key[0] === SESSION_PLUGINS_KEY;
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
