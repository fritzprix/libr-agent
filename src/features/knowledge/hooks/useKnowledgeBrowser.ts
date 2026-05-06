import { useCallback, useEffect, useMemo, useState } from 'react';
import { useKnowledgeDelete } from './useKnowledgeDelete';
import { useKnowledgeDetail } from './useKnowledgeDetail';
import { useKnowledgeList } from './useKnowledgeList';

export function useKnowledgeBrowser() {
  const [query, setQuery] = useState('');
  const [assistantFilter, setAssistantFilter] = useState('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [listRefreshToken, setListRefreshToken] = useState(0);
  const {
    assistants,
    hasMoreItems,
    isListLoading,
    isLoadingMore,
    items,
    loadMore,
  } = useKnowledgeList({
    assistantFilter,
    query,
    refreshToken: listRefreshToken,
  });
  const { detail, isDetailLoading } = useKnowledgeDetail(selectedId);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );

  const refresh = useCallback(() => {
    setListRefreshToken((current) => current + 1);
  }, []);

  useEffect(() => {
    if (selectedId !== null && !items.some((item) => item.id === selectedId)) {
      setSelectedId(null);
    }
  }, [items, selectedId]);

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
    assistants,
    cancelDelete,
    closeDetail,
    deleteSelectedItem,
    detail,
    hasMoreItems,
    isDeleteConfirming,
    isDeleting,
    isDetailLoading,
    isDetailOpen: selectedItem !== null,
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
    setQuery,
  };
}
