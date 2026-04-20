import useSWRInfinite from 'swr/infinite';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import {
  listGlobalKnowledge,
  type KnowledgeChunkListItem,
  type GlobalKnowledgeListResponse,
  type KnowledgeListCursor,
} from '@/lib/backend/knowledge';
import { useCallback, useMemo } from 'react';

const logger = getLogger('useKnowledgeList');

type FetcherArgs = [
  key: string,
  query: string,
  assistantId: string,
  limit: number,
  cursor: KnowledgeListCursor | null,
];

export function useKnowledgeList(
  debouncedQuery: string,
  assistantFilter: string,
  limit: number,
) {
  const { t } = useTranslation('common');

  const getKey = useCallback(
    (
      pageIndex: number,
      previousPageData: GlobalKnowledgeListResponse | null,
    ): FetcherArgs | null => {
      const keyBase: [string, string, string, number] = [
        'globalKnowledge',
        debouncedQuery,
        assistantFilter,
        limit,
      ];

      if (pageIndex === 0) return [...keyBase, null];

      if (!previousPageData?.nextCursor) return null;

      return [...keyBase, previousPageData.nextCursor];
    },
    [debouncedQuery, assistantFilter, limit],
  );

  const fetcher = useCallback(
    async ([
      ,
      query,
      assistantId,
      lim,
      cursor,
    ]: FetcherArgs): Promise<GlobalKnowledgeListResponse> => {
      return listGlobalKnowledge({
        query: query || undefined,
        assistantId: assistantId === 'all' ? undefined : assistantId,
        limit: lim,
        cursor: cursor ?? undefined,
      });
    },
    [],
  );

  const { data, error, size, setSize, isValidating, mutate } = useSWRInfinite(
    getKey,
    fetcher,
    {
      revalidateOnFocus: false,
      revalidateFirstPage: false,
      onError: (err) => {
        logger.error('Failed to load global knowledge list', err);
        toast.error(
          t('knowledge.loadListFailed', 'Failed to load global knowledge entries.'),
        );
      },
    }
  );

  const items = useMemo(() => {
    if (!data) return [];
    const allItems: KnowledgeChunkListItem[] = [];
    const seenIds = new Set<number>();

    for (const page of data) {
      for (const item of page.items) {
        if (!seenIds.has(item.id)) {
          seenIds.add(item.id);
          allItems.push(item);
        }
      }
    }
    return allItems;
  }, [data]);

  const assistants = useMemo(() => {
    if (!data || data.length === 0) return [];
    return data[0].assistants || [];
  }, [data]);

  const nextCursor = useMemo(() => {
    if (!data || data.length === 0) return null;
    return data[data.length - 1].nextCursor ?? null;
  }, [data]);

  const isLoadingMore =
    isValidating && size > 1 && data && typeof data[size - 1] === 'undefined';
  const isListLoading = !data && !error && isValidating;

  const loadMore = useCallback(() => {
    if (isLoadingMore || !nextCursor || isListLoading) return;
    setSize(size + 1);
  }, [isLoadingMore, nextCursor, isListLoading, setSize, size]);

  return {
    items,
    assistants,
    nextCursor,
    isListLoading,
    isLoadingMore,
    loadMore,
    mutateList: mutate,
  };
}
