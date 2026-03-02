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

  // Reset when agentId changes or dropdown closes.
  useEffect(() => {
    if (query === null) {
      setPlaybooks([]);
    }
  }, [agentId, query]);

  useEffect(() => {
    if (!agentId || query === null) return;

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

  return playbooks;
}
