import { useEffect, useState } from 'react';
import {
  listAssistantSummaries,
  type AssistantSummary,
} from '@/lib/backend/assistants';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAssistantSummaries');

export function useAssistantSummaries() {
  const [assistants, setAssistants] = useState<AssistantSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    let active = true;

    const load = async () => {
      try {
        setLoading(true);
        const summaries = await listAssistantSummaries();
        if (!active) {
          return;
        }
        setAssistants(summaries);
        setError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }
        const nextError =
          loadError instanceof Error
            ? loadError
            : new Error('Failed to load assistant summaries');
        logger.error('Failed to load assistant summaries', loadError);
        setAssistants([]);
        setError(nextError);
      } finally {
        if (active) {
          setLoading(false);
        }
      }
    };

    void load();

    return () => {
      active = false;
    };
  }, []);

  return { assistants, loading, error };
}
