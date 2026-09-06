import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  getGlobalKnowledgeGraph,
  type GlobalKnowledgeGraphResponse,
} from '@/lib/backend/knowledge';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useGlobalKnowledgeGraph');

export interface UseGlobalKnowledgeGraphOptions {
  assistantFilter?: string;
  limit?: number;
  enabled?: boolean;
}

export function useGlobalKnowledgeGraph(
  filterOrOptions: string | UseGlobalKnowledgeGraphOptions = 'all',
) {
  const { t } = useTranslation('common');
  const assistantFilter =
    typeof filterOrOptions === 'string'
      ? filterOrOptions
      : (filterOrOptions?.assistantFilter ?? 'all');
  const limit =
    typeof filterOrOptions === 'object' ? filterOrOptions?.limit : undefined;
  const enabled =
    typeof filterOrOptions === 'object' && filterOrOptions !== null
      ? (filterOrOptions.enabled ?? true)
      : true;

  const [graphData, setGraphData] =
    useState<GlobalKnowledgeGraphResponse | null>(null);
  const [isLoading, setIsLoading] = useState(enabled);
  const [error, setError] = useState<Error | null>(null);
  const [refreshToken, setRefreshToken] = useState(0);

  const refetch = useCallback(() => {
    setRefreshToken((prev) => prev + 1);
  }, []);

  useEffect(() => {
    if (!enabled) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    const fetchGraph = async () => {
      setIsLoading(true);
      setError(null);

      try {
        const resolvedFilter =
          assistantFilter === 'all' ? undefined : assistantFilter;
        const response = await getGlobalKnowledgeGraph(resolvedFilter, limit);

        if (!cancelled) {
          setGraphData(response);
        }
      } catch (err) {
        const normalizedError =
          err instanceof Error ? err : new Error(String(err));
        logger.error('Failed to load global knowledge graph', {
          error: normalizedError,
          assistantFilter,
        });

        if (!cancelled) {
          setError(normalizedError);
          toast.error(
            t(
              'knowledge.loadGraphFailed',
              'Failed to load knowledge graph data.',
            ),
          );
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    void fetchGraph();

    return () => {
      cancelled = true;
    };
  }, [assistantFilter, enabled, limit, refreshToken, t]);

  return {
    graphData,
    isLoading,
    error,
    refetch,
  };
}
