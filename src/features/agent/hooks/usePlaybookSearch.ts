import useSWR from 'swr';
import { listPlaybooks } from '@/lib/backend/playbooks';
import type { Playbook } from '@/types/playbook';
import { getLogger } from '@/lib/logger';

const logger = getLogger('usePlaybookSearch');

const MAX_RESULTS = 8;
type PlaybookSearchKey = readonly ['playbooks-search', string, string];

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
  const swrKey: PlaybookSearchKey | null =
    agentId && query !== null ? ['playbooks-search', agentId, query] : null;

  const { data: playbooks = [] } = useSWR<
    PlaybookWithId[],
    Error,
    PlaybookSearchKey | null
  >(
    swrKey,
    async ([, id, q]) => {
      const all = await listPlaybooks({ agentId: id });
      const lower = q.toLowerCase();
      return all
        .filter((p) => q === '' || p.goal.toLowerCase().includes(lower))
        .slice(0, MAX_RESULTS);
    },
    {
      revalidateOnFocus: false,
      onError: (err) => {
        logger.error('Failed to fetch playbooks for autocomplete', err);
      },
    },
  );

  // Dynamically return empty if query is null, avoiding any stale renders.
  if (query === null) return [];

  return playbooks;
}
