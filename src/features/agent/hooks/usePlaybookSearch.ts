import { useEffect, useState } from 'react';
import { listPlaybooks } from '@/lib/backend/playbooks';
import type { Playbook } from '@/types/playbook';
import { getLogger } from '@/lib/logger';

const logger = getLogger('usePlaybookSearch');

const MAX_RESULTS = 8;

type PlaybookWithId = Playbook & {
  id: string;
  createdAt: Date;
  updatedAt: Date;
};

/**
 * Fetches and filters playbooks for `@playbook:` autocomplete.
 * Pass `null` for query when the dropdown is not active — this resets cached results.
 *
 * Playbooks are scoped to the given assistant (agentId).
 */
export function usePlaybookSearch(
  agentId: string | undefined,
  query: string | null,
): PlaybookWithId[] {
  const [playbooks, setPlaybooks] = useState<PlaybookWithId[]>([]);

  useEffect(() => {
    if (!agentId || query === null) {
      // If we are closed or missing agentId, we don't need to retain playbooks state,
      // but dynamically returning [] at the end handles the rendering.
      // We also clear it here so it doesn't flash old data on the next open.
      setPlaybooks([]);
      return;
    }

    listPlaybooks({ agentId })
      .then((all) => {
        const lower = query.toLowerCase();
        const filtered = all
          .filter((p) => query === '' || p.goal.toLowerCase().includes(lower))
          .slice(0, MAX_RESULTS);
        setPlaybooks(filtered);
      })
      .catch((err) => {
        logger.error('Failed to fetch playbooks for autocomplete', err);
        setPlaybooks([]);
      });
  }, [agentId, query]);

  // Dynamically return empty if query is null, avoiding any stale renders.
  if (query === null) return [];

  return playbooks;
}
