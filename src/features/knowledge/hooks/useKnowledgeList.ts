import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import {
  listGlobalKnowledge,
  type KnowledgeChunkListItem,
  type KnowledgeListCursor,
} from '@/lib/backend/knowledge';
import { useDebouncedValue } from './useDebouncedValue';

const logger = getLogger('useKnowledgeList');
const KNOWLEDGE_PAGE_SIZE = 60;

function buildKnowledgeRequestKey(
  assistantFilter: string,
  normalizedQuery: string,
  refreshToken: number,
): string {
  return `${assistantFilter}::${normalizedQuery}::${refreshToken}`;
}

interface UseKnowledgeListOptions {
  assistantFilter: string;
  query: string;
  refreshToken: number;
}

export function useKnowledgeList({
  assistantFilter,
  query,
  refreshToken,
}: UseKnowledgeListOptions) {
  const { t } = useTranslation('common');
  const [items, setItems] = useState<KnowledgeChunkListItem[]>([]);
  const [assistants, setAssistants] = useState<string[]>([]);
  const [isListLoading, setIsListLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [nextCursor, setNextCursor] = useState<KnowledgeListCursor | null>(
    null,
  );
  const debouncedQuery = useDebouncedValue(query, 250);
  const normalizedQuery = debouncedQuery.trim();
  const requestKey = buildKnowledgeRequestKey(
    assistantFilter,
    normalizedQuery,
    refreshToken,
  );
  const latestRequestKeyRef = useRef(requestKey);
  latestRequestKeyRef.current = requestKey;

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setIsListLoading(true);
      try {
        const response = await listGlobalKnowledge({
          query: normalizedQuery || undefined,
          assistantId: assistantFilter === 'all' ? undefined : assistantFilter,
          limit: KNOWLEDGE_PAGE_SIZE,
        });

        if (cancelled) {
          return;
        }

        setItems(response.items);
        setAssistants(response.assistants);
        setNextCursor(response.nextCursor ?? null);
      } catch (error) {
        logger.error('Failed to load global knowledge list', error);
        if (!cancelled) {
          toast.error(
            t(
              'knowledge.loadListFailed',
              'Failed to load global knowledge entries.',
            ),
          );
        }
      } finally {
        if (!cancelled) {
          setIsListLoading(false);
        }
      }
    };

    void load();

    return () => {
      cancelled = true;
    };
  }, [assistantFilter, normalizedQuery, refreshToken, t]);

  const loadMore = useCallback(async () => {
    if (isListLoading || isLoadingMore || nextCursor === null) {
      return;
    }

    const requestKeyAtInvocation = latestRequestKeyRef.current;
    setIsLoadingMore(true);
    try {
      const response = await listGlobalKnowledge({
        query: normalizedQuery || undefined,
        assistantId: assistantFilter === 'all' ? undefined : assistantFilter,
        cursor: nextCursor,
        limit: KNOWLEDGE_PAGE_SIZE,
      });

      if (latestRequestKeyRef.current !== requestKeyAtInvocation) {
        return;
      }

      setItems((current) => {
        const seenIds = new Set(current.map((item) => item.id));
        const appendedItems = response.items.filter(
          (item) => !seenIds.has(item.id),
        );
        return [...current, ...appendedItems];
      });
      setNextCursor(response.nextCursor ?? null);
    } catch (error) {
      logger.error('Failed to load more global knowledge entries', error);
      toast.error(
        t('knowledge.loadMoreFailed', 'Failed to load more knowledge entries.'),
      );
    } finally {
      setIsLoadingMore(false);
    }
  }, [
    assistantFilter,
    isListLoading,
    isLoadingMore,
    nextCursor,
    normalizedQuery,
    t,
  ]);

  return {
    assistants,
    hasMoreItems: nextCursor !== null,
    isListLoading,
    isLoadingMore,
    items,
    loadMore,
  };
}
