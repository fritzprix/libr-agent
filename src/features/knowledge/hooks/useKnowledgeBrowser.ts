import { useCallback, useMemo, useState } from 'react';
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

  const [prevItems, setPrevItems] = useState(items);
  if (items !== prevItems) {
    setPrevItems(items);
    if (selectedId !== null && !items.some((item) => item.id === selectedId)) {
      setSelectedId(null);
    }
  }

  const handleDeleted = useCallback(() => {
    setSelectedId(null);
    setListRefreshToken((current) => current + 1);
  }, []);
  const {
    deleteSelectedItem,
    isDeleteDialogOpen,
    isDeleting,
    requestDelete,
    setIsDeleteDialogOpen,
  } = useKnowledgeDelete({
    onDeleted: handleDeleted,
    selectedItem,
  });

  const closeDetail = useCallback(() => {
    setIsDeleteDialogOpen(false);
    setSelectedId(null);
  }, [setIsDeleteDialogOpen]);

  const selectItem = useCallback((id: number) => {
    setSelectedId(id);
  }, []);

  return {
    assistantFilter,
    assistants,
    closeDetail,
    deleteSelectedItem,
    detail,
    hasMoreItems,
    isDeleteDialogOpen,
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
    setIsDeleteDialogOpen,
    setQuery,
  };
}
