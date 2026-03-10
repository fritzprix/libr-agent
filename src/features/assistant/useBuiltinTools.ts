import { useState, useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import type { BuiltinServerInfo } from '@/lib/backend/types';

const logger = getLogger('useBuiltinTools');

export function useBuiltinTools() {
  const [services, setServices] = useState<BuiltinServerInfo[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;

    async function fetchDefinitions() {
      try {
        const defs = await listAvailableBuiltinServerDefinitions();
        if (isMounted) {
          setServices(
            defs.sort((a, b) =>
              a.metadata.displayName.localeCompare(b.metadata.displayName),
            ),
          );
          setIsLoading(false);
        }
      } catch (err) {
        logger.error('Failed to fetch builtin server definitions', err);
        if (isMounted) setIsLoading(false);
      }
    }

    fetchDefinitions();

    return () => {
      isMounted = false;
    };
  }, []);

  return { services, isLoading };
}
