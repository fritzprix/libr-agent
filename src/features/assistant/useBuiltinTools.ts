import useSWR from 'swr';
import { getLogger } from '@/lib/logger';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import type { BuiltinServerInfo } from '@/lib/backend/types';

const logger = getLogger('useBuiltinTools');

export function useBuiltinTools() {
  const { data: services = [], isLoading } = useSWR<BuiltinServerInfo[], Error>(
    'builtin-tools',
    async () => {
      const defs = await listAvailableBuiltinServerDefinitions();
      return defs.sort((a, b) =>
        a.metadata.displayName.localeCompare(b.metadata.displayName),
      );
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to fetch builtin server definitions', err);
      },
    },
  );

  return { services, isLoading };
}
