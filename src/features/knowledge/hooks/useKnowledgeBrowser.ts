import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
  useTransition,
} from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import {
  deleteGlobalKnowledge,
  getGlobalKnowledgeDetail,
  listGlobalKnowledge,
  type KnowledgeChunkDetail,
  type KnowledgeChunkListItem,
  type KnowledgeListCursor,
} from '@/lib/backend/knowledge';

const logger = getLogger('useKnowledgeBrowser');
const KNOWLEDGE_PAGE_SIZE = 60;

export function useKnowledgeBrowser() {
  const { t } = useTranslation('common');
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [assistantFilter, setAssistantFilter] = useState('all');
  const [items, setItems] = useState<KnowledgeChunkListItem[]>([]);
  const [assistants, setAssistants] = useState<string[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [detail, setDetail] = useState<KnowledgeChunkDetail | null>(null);
  const [isListLoading, setIsListLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isDetailLoading, setIsDetailLoading] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const [listRefreshToken, setListRefreshToken] = useState(0);
  const [nextCursor, setNextCursor] = useState<KnowledgeListCursor | null>(
    null,
  );
  const deferredQuery = useDeferredValue(query);
  const [, startSelectionTransition] = useTransition();

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setDebouncedQuery(deferredQuery.trim());
    }, 250);

    return () => window.clearTimeout(timeout);
  }, [deferredQuery]);

  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setIsListLoading(true);
      try {
        const response = await listGlobalKnowledge({
          query: debouncedQuery || undefined,
          assistantId: assistantFilter === 'all' ? undefined : assistantFilter,
          limit: KNOWLEDGE_PAGE_SIZE,
        });

        if (cancelled) {
          return;
        }

        setItems(response.items);
        setAssistants(response.assistants);
        setNextCursor(response.nextCursor ?? null);
        setSelectedId((current) => {
          if (current && response.items.some((item) => item.id === current)) {
            return current;
          }
          return null;
        });
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
  }, [assistantFilter, debouncedQuery, listRefreshToken, t]);

  useEffect(() => {
    if (selectedId === null) {
      setDetail(null);
      return;
    }

    let cancelled = false;

    const loadDetail = async () => {
      setIsDetailLoading(true);
      try {
        const response = await getGlobalKnowledgeDetail(selectedId);
        if (!cancelled) {
          setDetail(response);
        }
      } catch (error) {
        logger.error('Failed to load knowledge detail', { selectedId, error });
        if (!cancelled) {
          toast.error(
            t(
              'knowledge.loadDetailFailed',
              'Failed to load knowledge details.',
            ),
          );
          setDetail(null);
        }
      } finally {
        if (!cancelled) {
          setIsDetailLoading(false);
        }
      }
    };

    void loadDetail();

    return () => {
      cancelled = true;
    };
  }, [selectedId, t]);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );
  const isDetailOpen = selectedItem !== null;
  const hasMoreItems = nextCursor !== null;
  const entityNameById = useMemo(
    () =>
      new Map(detail?.entities.map((entity) => [entity.id, entity.name]) ?? []),
    [detail?.entities],
  );

  const refresh = useCallback(() => {
    setListRefreshToken((current) => current + 1);
  }, []);

  const closeDetail = useCallback(() => {
    setIsDeleteDialogOpen(false);
    setSelectedId(null);
  }, []);

  const selectItem = useCallback(
    (id: number) => {
      startSelectionTransition(() => {
        setSelectedId(id);
      });
    },
    [startSelectionTransition],
  );

  const loadMore = useCallback(async () => {
    if (isListLoading || isLoadingMore || nextCursor === null) {
      return;
    }

    setIsLoadingMore(true);
    try {
      const response = await listGlobalKnowledge({
        query: debouncedQuery || undefined,
        assistantId: assistantFilter === 'all' ? undefined : assistantFilter,
        cursor: nextCursor,
        limit: KNOWLEDGE_PAGE_SIZE,
      });

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
    debouncedQuery,
    isListLoading,
    isLoadingMore,
    nextCursor,
    t,
  ]);

  const requestDelete = useCallback(() => {
    if (!selectedItem || isDeleting) {
      return;
    }
    setIsDeleteDialogOpen(true);
  }, [isDeleting, selectedItem]);

  const deleteSelectedItem = useCallback(async () => {
    if (!selectedItem || isDeleting) {
      return;
    }

    setIsDeleting(true);
    try {
      const response = await deleteGlobalKnowledge(selectedItem.id);
      toast.success(t('knowledge.deleteSuccess', 'Knowledge entry deleted.'), {
        description: t(
          'knowledge.deleteSuccessDescription',
          'Removed {{entities}} orphan entities and {{relationships}} orphan relationships.',
          {
            entities: response.orphanEntityCount,
            relationships: response.orphanRelationshipCount,
          },
        ),
      });
      setDetail(null);
      setSelectedId(null);
      setIsDeleteDialogOpen(false);
      setListRefreshToken((current) => current + 1);
    } catch (error) {
      logger.error('Failed to delete knowledge entry', {
        id: selectedItem.id,
        error,
      });
      toast.error(
        t('knowledge.deleteFailed', 'Failed to delete knowledge entry.'),
      );
    } finally {
      setIsDeleting(false);
    }
  }, [isDeleting, selectedItem, t]);

  return {
    assistantFilter,
    assistants,
    closeDetail,
    deleteSelectedItem,
    detail,
    entityNameById,
    hasMoreItems,
    isDeleteDialogOpen,
    isDeleting,
    isDetailLoading,
    isDetailOpen,
    isListLoading,
    isLoadingMore,
    items,
    loadMore,
    query,
    refresh,
    requestDelete,
    selectItem,
    selectedId,
    selectedItem,
    setAssistantFilter,
    setIsDeleteDialogOpen,
    setQuery,
  };
}
