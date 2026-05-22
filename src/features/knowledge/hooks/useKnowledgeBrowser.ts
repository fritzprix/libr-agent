import { useCallback, useMemo, useState } from 'react';
import { useKnowledgeDelete } from './useKnowledgeDelete';
import { useKnowledgeDetail } from './useKnowledgeDetail';
import { useKnowledgeList } from './useKnowledgeList';
import { useAssistantSummaries } from '@/features/agent/hooks/useAssistantSummaries';

interface KnowledgeAssistantOption {
  id: string;
  label: string;
}

export function useKnowledgeBrowser() {
  const [query, setQuery] = useState('');
  const [assistantFilter, setAssistantFilter] = useState('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [listRefreshToken, setListRefreshToken] = useState(0);

  const { assistants: assistantSummaries } = useAssistantSummaries();
  const {
    assistants,
    hasMoreItems,
    isInitialListLoading,
    isListLoading,
    isLoadingMore,
    isRefreshingList,
    items,
    loadMore,
  } = useKnowledgeList({
    assistantFilter,
    query,
    refreshToken: listRefreshToken,
  });

  const [prevItems, setPrevItems] = useState(items);
  if (items !== prevItems) {
    setPrevItems(items);
    if (selectedId !== null && !items.some((item) => item.id === selectedId)) {
      setSelectedId(null);
    }
  }

  const { detail, isDetailLoading } = useKnowledgeDetail(selectedId);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );

  const assistantOptions = useMemo<KnowledgeAssistantOption[]>(() => {
    const labelById = new Map(
      assistantSummaries.map((assistant) => [assistant.id, assistant.name]),
    );
    const optionIds = new Set(assistants);

    if (assistantFilter !== 'all') {
      optionIds.add(assistantFilter);
    }

    return [...optionIds]
      .map((id) => ({
        id,
        label: labelById.get(id) ?? id,
      }))
      .sort((left, right) => left.label.localeCompare(right.label));
  }, [assistantFilter, assistantSummaries, assistants]);

  const refresh = useCallback(() => {
    setListRefreshToken((current) => current + 1);
  }, []);

  const handleDeleted = useCallback(() => {
    setSelectedId(null);
    setListRefreshToken((current) => current + 1);
  }, []);
  const {
    cancelDelete,
    deleteSelectedItem,
    isDeleteConfirming,
    isDeleting,
    requestDelete,
  } = useKnowledgeDelete({
    onDeleted: handleDeleted,
    selectedItem,
  });

  const closeDetail = useCallback(() => {
    cancelDelete();
    setSelectedId(null);
  }, [cancelDelete]);

  const selectItem = useCallback((id: number) => {
    setSelectedId(id);
  }, []);

  return {
    assistantFilter,
    assistantOptions,
    cancelDelete,
    closeDetail,
    deleteSelectedItem,
    detail,
    hasMoreItems,
    isDeleteConfirming,
    isDeleting,
    isDetailLoading,
    isDetailOpen: selectedItem !== null,
    isInitialListLoading,
    isListLoading,
    isLoadingMore,
    isRefreshingList,
    items,
    loadMore,
    query,
    refresh,
    requestDelete,
    selectItem,
    selectedId,
    selectedItem,
    setAssistantFilter,
    setQuery,
  };
}
