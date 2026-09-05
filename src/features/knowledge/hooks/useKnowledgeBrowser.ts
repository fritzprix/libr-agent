import { useCallback, useMemo, useState } from 'react';
import { useKnowledgeDelete } from './useKnowledgeDelete';
import { useKnowledgeDetail } from './useKnowledgeDetail';
import { useKnowledgeList } from './useKnowledgeList';
import { useGlobalKnowledgeGraph } from './useGlobalKnowledgeGraph';
import { useAssistantSummaries } from '@/features/agent/hooks/useAssistantSummaries';

export type KnowledgeViewMode = 'split' | 'graph' | 'cards';

interface KnowledgeAssistantOption {
  id: string;
  label: string;
}

export function useKnowledgeBrowser() {
  const [viewMode, setViewMode] = useState<KnowledgeViewMode>('split');
  const [query, setQuery] = useState('');
  const [assistantFilter, setAssistantFilter] = useState('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [selectedEntityId, setSelectedEntityId] = useState<number | null>(null);
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

  const {
    graphData: rawGraphData,
    isLoading: isGraphLoading,
    error: graphError,
    refetch: refetchGraph,
  } = useGlobalKnowledgeGraph({
    assistantFilter,
    enabled: viewMode !== 'cards',
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

  const selectedEntity = useMemo(
    () =>
      rawGraphData?.entities.find((entity) => entity.id === selectedEntityId) ??
      detail?.entities.find((entity) => entity.id === selectedEntityId) ??
      null,
    [rawGraphData, detail, selectedEntityId],
  );

  const graphData = useMemo(() => {
    if (!rawGraphData && !detail) return null;

    const baseEntities = rawGraphData?.entities ?? [];
    const baseRels = rawGraphData?.relationships ?? [];

    if (!detail?.relationships || detail.relationships.length === 0) {
      return rawGraphData;
    }

    const isFromDetail =
      selectedEntity !== null &&
      !baseEntities.some((e) => e.id === selectedEntity.id);

    if (isFromDetail || !rawGraphData) {
      const existingRelIds = new Set(baseRels.map((r) => r.id));
      const additionalRels = detail.relationships.filter(
        (r) => !existingRelIds.has(r.id),
      );

      const existingEntityIds = new Set(baseEntities.map((e) => e.id));
      const additionalEntities = (detail.entities ?? []).filter(
        (e) => !existingEntityIds.has(e.id),
      );

      return {
        entities: [...baseEntities, ...additionalEntities],
        relationships: [...baseRels, ...additionalRels],
      };
    }

    return rawGraphData;
  }, [rawGraphData, detail, selectedEntity]);

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
    refetchGraph();
  }, [refetchGraph]);

  const handleDeleted = useCallback(() => {
    setSelectedId(null);
    setListRefreshToken((current) => current + 1);
    refetchGraph();
  }, [refetchGraph]);

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

  const selectItem = useCallback((id: number | null) => {
    setSelectedId(id);
  }, []);

  const selectEntity = useCallback((id: number | null) => {
    setSelectedEntityId(id);
  }, []);

  return {
    assistantFilter,
    assistantOptions,
    cancelDelete,
    closeDetail,
    deleteSelectedItem,
    detail,
    graphData,
    graphError,
    hasMoreItems,
    isDeleteConfirming,
    isDeleting,
    isDetailLoading,
    isDetailOpen: selectedItem !== null,
    isGraphLoading,
    isInitialListLoading,
    isListLoading,
    isLoadingMore,
    isRefreshingList,
    items,
    loadMore,
    query,
    refresh,
    refetchGraph,
    requestDelete,
    selectEntity,
    selectedEntity,
    selectedEntityId,
    selectItem,
    selectedId,
    selectedItem,
    setAssistantFilter,
    setQuery,
    setViewMode,
    viewMode,
  };
}
