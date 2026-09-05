import { forwardRef, useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Columns2,
  Database,
  Filter,
  LayoutGrid,
  Loader2,
  Network,
  RefreshCw,
  Search,
  X,
} from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui';
import { Skeleton } from '@/components/ui/skeleton';
import {
  type GridComponents,
  type GridItemProps,
  type GridListProps,
  type GridScrollSeekPlaceholderProps,
  Virtuoso,
  VirtuosoGrid,
} from 'react-virtuoso';
import { cn } from '@/lib/utils';
import type {
  KnowledgeChunkListItem,
  KnowledgeGraphEntity,
  KnowledgeGraphRelationship,
} from '@/lib/backend/knowledge';
import { KnowledgeListItemCard } from './components/KnowledgeListItemCard';
import { KnowledgeNetworkCanvas } from './components/graph';
import {
  KnowledgeInspectorSheet,
  type InspectedTarget,
} from './components/KnowledgeInspectorSheet';
import { useKnowledgeBrowser } from './hooks/useKnowledgeBrowser';

const EMPTY_GRAPH_ENTITIES: KnowledgeGraphEntity[] = [];
const EMPTY_GRAPH_RELATIONSHIPS: KnowledgeGraphRelationship[] = [];

interface KnowledgeGridContext {
  endOfResultsLabel: string;
  excerptLabel: string;
  hasMoreItems: boolean;
  isLoadingMore: boolean;
  loadMoreLabel: string;
  onLoadMore: () => void;
  onSelect: (id: number) => void;
  selectedId: number | null;
  untitledLabel: string;
}

const knowledgeGridComponents: GridComponents<KnowledgeGridContext> = {
  List: forwardRef<HTMLDivElement, GridListProps>(function KnowledgeGridList(
    { children, className, style, ...props },
    ref,
  ) {
    return (
      <div
        {...props}
        ref={ref}
        className={cn('flex flex-wrap content-start px-1', className)}
        style={style}
      >
        {children}
      </div>
    );
  }),
  Item: forwardRef<HTMLDivElement, GridItemProps>(function KnowledgeGridItem(
    { children, className, style, ...props },
    ref,
  ) {
    return (
      <div
        {...props}
        ref={ref}
        className={cn('flex w-full p-1.5 lg:w-1/2 2xl:w-1/3', className)}
        style={style}
      >
        {children}
      </div>
    );
  }),
  Footer: function KnowledgeGridFooter({ context }) {
    if (!context.hasMoreItems) {
      return (
        <div className="px-3 py-4 text-center text-xs text-muted-foreground">
          {context.endOfResultsLabel}
        </div>
      );
    }

    return (
      <div className="flex justify-center px-3 py-4">
        <Button
          type="button"
          variant="outline"
          onClick={context.onLoadMore}
          disabled={context.isLoadingMore}
          className="min-w-36 gap-2"
        >
          {context.isLoadingMore ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : null}
          {context.loadMoreLabel}
        </Button>
      </div>
    );
  },
  ScrollSeekPlaceholder: function KnowledgeGridScrollSeekPlaceholder({
    height,
  }: GridScrollSeekPlaceholderProps) {
    return (
      <div
        className="w-full rounded-2xl border border-border/60 p-4"
        style={{ height }}
      >
        <div className="space-y-2">
          <Skeleton className="h-4 w-24" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-3 w-4/5" />
        </div>
      </div>
    );
  },
};

export default function KnowledgePage() {
  const { t } = useTranslation('common');
  const {
    assistantFilter,
    assistantOptions,
    cancelDelete,
    closeDetail,
    deleteSelectedItem,
    detail,
    graphData,
    hasMoreItems,
    isDeleteConfirming,
    isDeleting,
    isDetailLoading,
    isGraphLoading,
    isInitialListLoading,
    isListLoading,
    isLoadingMore,
    isRefreshingList,
    items,
    loadMore,
    query,
    refresh,
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
  } = useKnowledgeBrowser();

  const [inspectTarget, setInspectTarget] = useState<InspectedTarget | null>(null);
  const [isStreamFilteredByEntity, setIsStreamFilteredByEntity] = useState(false);

  const excerptLabel = t('knowledge.excerpt', 'Excerpt');
  const untitledLabel = t(
    'knowledge.untitledEntry',
    'Untitled knowledge entry',
  );
  const loadMoreLabel = t('knowledge.loadMore', 'Load more');
  const endOfResultsLabel = t(
    'knowledge.endOfResults',
    'You have reached the end of the current results.',
  );

  // Clean up inspector target if inspected chunk was deleted
  useEffect(() => {
    if (
      inspectTarget?.type === 'chunk' &&
      items.length > 0 &&
      !items.some((item) => item.id === inspectTarget.id)
    ) {
      setInspectTarget(null);
    }
  }, [items, inspectTarget]);

  // Handle selecting a chunk from card click
  const handleSelectChunk = useCallback(
    (id: number) => {
      selectItem(id);
      setInspectTarget({ type: 'chunk', id });
    },
    [selectItem],
  );

  // Handle selecting an entity from canvas node click
  const handleSelectEntityFromCanvas = useCallback(
    (entityId: number | null) => {
      selectEntity(entityId);
      if (entityId !== null) {
        setInspectTarget({ type: 'entity', id: entityId });
      } else {
        setInspectTarget((prev) => (prev?.type === 'entity' ? null : prev));
      }
    },
    [selectEntity],
  );

  // Handle selecting an entity from within the inspector sheet
  const handleSelectEntityFromInspector = useCallback(
    (entityId: number) => {
      selectEntity(entityId);
      setInspectTarget({ type: 'entity', id: entityId });
    },
    [selectEntity],
  );

  // Close inspector sheet
  const handleCloseInspector = useCallback(() => {
    setInspectTarget(null);
    closeDetail();
    selectEntity(null);
  }, [closeDetail, selectEntity]);

  const handleLoadMore = useCallback(() => {
    void loadMore();
  }, [loadMore]);

  // Set of chunk IDs that match the currently selected entity name or tags
  const entityMatchingChunkIds = useMemo(() => {
    if (!selectedEntity) return new Set<number>();
    const nameLower = selectedEntity.name.toLowerCase();
    const ids = new Set<number>();
    for (const item of items) {
      if (
        item.preview.toLowerCase().includes(nameLower) ||
        item.tags.some((tag) => tag.toLowerCase().includes(nameLower))
      ) {
        ids.add(item.id);
      }
    }
    return ids;
  }, [selectedEntity, items]);

  // Chunks to display in stream (optionally filtered by selected entity)
  const streamItems = useMemo(() => {
    if (isStreamFilteredByEntity && selectedEntity) {
      return items.filter((item) => entityMatchingChunkIds.has(item.id));
    }
    return items;
  }, [items, isStreamFilteredByEntity, selectedEntity, entityMatchingChunkIds]);

  const gridContext = useMemo<KnowledgeGridContext>(
    () => ({
      endOfResultsLabel,
      excerptLabel,
      hasMoreItems,
      isLoadingMore,
      loadMoreLabel,
      onLoadMore: handleLoadMore,
      onSelect: handleSelectChunk,
      selectedId: inspectTarget?.type === 'chunk' ? selectedId : null,
      untitledLabel,
    }),
    [
      endOfResultsLabel,
      excerptLabel,
      hasMoreItems,
      isLoadingMore,
      loadMoreLabel,
      handleLoadMore,
      handleSelectChunk,
      inspectTarget,
      selectedId,
      untitledLabel,
    ],
  );

  const renderKnowledgeCard = useCallback(
    (
      _index: number,
      item: KnowledgeChunkListItem,
      context: KnowledgeGridContext,
    ) => (
      <KnowledgeListItemCard
        excerptLabel={context.excerptLabel}
        item={item}
        isActive={item.id === context.selectedId}
        onSelect={context.onSelect}
        untitledLabel={context.untitledLabel}
      />
    ),
    [],
  );

  return (
    <div className="h-full bg-background p-6">
      <div className="mx-auto flex h-full max-w-7xl 2xl:max-w-[1700px] flex-col gap-4">
        {/* Header Bar */}
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center rounded-xl bg-primary/10 p-2.5 text-primary">
              <Database size={28} />
            </div>
            <div>
              <h1 className="text-2xl font-semibold tracking-tight text-foreground">
                {t('knowledge.pageTitle', 'Global Knowledge')}
              </h1>
              <p className="mt-0.5 text-sm text-muted-foreground">
                {t(
                  'knowledge.pageSubtitle',
                  'Search shared memory, inspect evidence, and prune stale knowledge.',
                )}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2.5">
            {/* View Mode Switcher */}
            <div className="flex items-center rounded-xl border border-border/60 bg-muted/40 p-1 shadow-xs">
              <Button
                type="button"
                variant={viewMode === 'split' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setViewMode('split')}
                className={cn(
                  'h-8 gap-1.5 px-3 text-xs font-medium',
                  viewMode === 'split' &&
                    'bg-background text-foreground shadow-xs',
                )}
              >
                <Columns2 className="h-3.5 w-3.5" />
                <span>{t('knowledge.viewMode.split', 'Split')}</span>
              </Button>
              <Button
                type="button"
                variant={viewMode === 'graph' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setViewMode('graph')}
                className={cn(
                  'h-8 gap-1.5 px-3 text-xs font-medium',
                  viewMode === 'graph' &&
                    'bg-background text-foreground shadow-xs',
                )}
              >
                <Network className="h-3.5 w-3.5" />
                <span>{t('knowledge.viewMode.graph', 'Graph')}</span>
              </Button>
              <Button
                type="button"
                variant={viewMode === 'cards' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setViewMode('cards')}
                className={cn(
                  'h-8 gap-1.5 px-3 text-xs font-medium',
                  viewMode === 'cards' &&
                    'bg-background text-foreground shadow-xs',
                )}
              >
                <LayoutGrid className="h-3.5 w-3.5" />
                <span>{t('knowledge.viewMode.cards', 'Cards')}</span>
              </Button>
            </div>

            {/* Refresh Button */}
            <Button
              type="button"
              variant="outline"
              onClick={refresh}
              disabled={isListLoading || isGraphLoading}
              className="h-9 gap-2"
            >
              <RefreshCw
                className={cn(
                  'h-4 w-4',
                  (isListLoading || isGraphLoading) && 'animate-spin',
                )}
              />
              {t('knowledge.refresh', 'Refresh')}
            </Button>
          </div>
        </div>

        {/* Toolbar: Search and Assistant Filter */}
        <div className="flex flex-wrap items-center gap-3">
          <div className="relative min-w-[240px] flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t(
                'knowledge.searchPlaceholder',
                'Search content, tags, or source...',
              )}
              className="h-9 pl-9"
            />
          </div>

          <div className="w-56 shrink-0">
            <Select
              value={assistantFilter}
              onValueChange={setAssistantFilter}
            >
              <SelectTrigger className="h-9">
                <SelectValue
                  placeholder={t(
                    'knowledge.assistantFilterPlaceholder',
                    'Filter by assistant',
                  )}
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">
                  {t('knowledge.assistantFilterAll', 'All assistants')}
                </SelectItem>
                {assistantOptions.map((assistant) => (
                  <SelectItem key={assistant.id} value={assistant.id}>
                    {assistant.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        </div>

        {isRefreshingList && (
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            {t('knowledge.refreshingResults', 'Updating results...')}
          </div>
        )}

        {/* Main Content Area with Responsive Multi-Mode Layout */}
        <div className="relative min-h-0 flex-1 overflow-hidden">
          {/* SPLIT MODE: Left Pane (Canvas ~65%), Right Pane (Curated Stream ~35%) */}
          {viewMode === 'split' && (
            <div className="flex h-full min-h-0 w-full gap-4">
              {/* Left Pane: Network Canvas */}
              <div className="relative min-h-0 min-w-0 flex-1 overflow-hidden">
                <KnowledgeNetworkCanvas
                  entities={graphData?.entities ?? EMPTY_GRAPH_ENTITIES}
                  relationships={
                    graphData?.relationships ?? EMPTY_GRAPH_RELATIONSHIPS
                  }
                  selectedEntityId={selectedEntityId}
                  onSelectEntity={handleSelectEntityFromCanvas}
                  isLoading={isGraphLoading && !graphData}
                  className="h-full w-full"
                />
              </div>

              {/* Right Pane: Curated Chunk Stream */}
              <div className="flex h-full min-h-0 w-[35%] min-w-[340px] max-w-[440px] flex-col rounded-2xl border border-border/60 bg-background/50 backdrop-blur-sm">
                {/* Stream Header */}
                <div className="flex shrink-0 flex-col gap-2 border-b border-border/60 p-3.5">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <h2 className="text-sm font-semibold text-foreground">
                        {t('knowledge.streamTitle', 'Curated Stream')}
                      </h2>
                      <Badge variant="secondary" className="text-xs">
                        {streamItems.length}
                      </Badge>
                    </div>

                    {selectedEntity && inspectTarget === null && (
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 text-xs"
                        onClick={() =>
                          setInspectTarget({
                            type: 'entity',
                            id: selectedEntity.id,
                          })
                        }
                      >
                        {t('knowledge.inspectEntity', 'Inspect Entity')}
                      </Button>
                    )}
                  </div>

                  {/* Selected Entity filter / highlight status indicator */}
                  {selectedEntity && (
                    <div className="flex items-center justify-between gap-2 rounded-lg border border-primary/30 bg-primary/5 px-2.5 py-1.5 text-xs text-primary">
                      <div className="flex min-w-0 items-center gap-1.5">
                        <Network className="h-3.5 w-3.5 shrink-0" />
                        <span className="truncate font-medium">
                          {selectedEntity.name}
                        </span>
                        <span className="text-[11px] text-muted-foreground">
                          ({entityMatchingChunkIds.size})
                        </span>
                      </div>
                      <div className="flex items-center gap-1">
                        <Button
                          type="button"
                          variant={isStreamFilteredByEntity ? 'default' : 'ghost'}
                          size="sm"
                          className="h-6 px-2 text-[11px]"
                          onClick={() => setIsStreamFilteredByEntity((v) => !v)}
                        >
                          <Filter className="mr-1 h-3 w-3" />
                          {isStreamFilteredByEntity
                            ? t('knowledge.filtered', 'Filtered')
                            : t('knowledge.filter', 'Filter')}
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          className="h-6 w-6 text-muted-foreground hover:text-foreground"
                          onClick={() => {
                            selectEntity(null);
                            setIsStreamFilteredByEntity(false);
                            if (inspectTarget?.type === 'entity') {
                              setInspectTarget(null);
                            }
                          }}
                        >
                          <X className="h-3 w-3" />
                        </Button>
                      </div>
                    </div>
                  )}
                </div>

                {/* Stream Body */}
                <div className="min-h-0 flex-1 p-2">
                  {isInitialListLoading ? (
                    <div className="space-y-3 p-2">
                      {Array.from({ length: 4 }).map((_, idx) => (
                        <div
                          key={idx}
                          className="space-y-2 rounded-xl border p-3"
                        >
                          <Skeleton className="h-4 w-24" />
                          <Skeleton className="h-3 w-full" />
                          <Skeleton className="h-3 w-4/5" />
                        </div>
                      ))}
                    </div>
                  ) : streamItems.length === 0 ? (
                    <div className="rounded-xl border border-dashed p-6 text-center text-xs text-muted-foreground">
                      {isStreamFilteredByEntity
                        ? t(
                            'knowledge.noMatchingChunksForEntity',
                            'No chunks match the selected entity.',
                          )
                        : t(
                            'knowledge.emptyState',
                            'No knowledge entries match the current filters.',
                          )}
                    </div>
                  ) : (
                    <Virtuoso
                      style={{ height: '100%' }}
                      data={streamItems}
                      computeItemKey={(_index, item) => item.id}
                      components={{
                        Footer: () =>
                          hasMoreItems && !isStreamFilteredByEntity ? (
                            <div className="flex justify-center p-3">
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={handleLoadMore}
                                disabled={isLoadingMore}
                                className="gap-1.5 text-xs"
                              >
                                {isLoadingMore && (
                                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                                )}
                                {loadMoreLabel}
                              </Button>
                            </div>
                          ) : null,
                      }}
                      itemContent={(_index, item) => {
                        const isHighlighted = entityMatchingChunkIds.has(item.id);
                        const isSelected =
                          item.id === selectedId &&
                          inspectTarget?.type === 'chunk';

                        return (
                          <div className="p-1.5">
                            <div
                              className={cn(
                                'relative rounded-2xl transition-all',
                                isHighlighted &&
                                  !isSelected &&
                                  'rounded-2xl ring-2 ring-primary/40',
                              )}
                            >
                              {isHighlighted && (
                                <div className="absolute right-3 top-3 z-10">
                                  <Badge
                                    variant="outline"
                                    className="border-primary/40 bg-primary/10 text-[10px] text-primary"
                                  >
                                    {t('knowledge.linked', 'Linked')}
                                  </Badge>
                                </div>
                              )}
                              <KnowledgeListItemCard
                                excerptLabel={excerptLabel}
                                item={item}
                                isActive={isSelected}
                                onSelect={handleSelectChunk}
                                untitledLabel={untitledLabel}
                              />
                            </div>
                          </div>
                        );
                      }}
                    />
                  )}
                </div>
              </div>
            </div>
          )}

          {/* GRAPH MODE: Full Canvas */}
          {viewMode === 'graph' && (
            <div className="h-full min-h-0 w-full">
              <KnowledgeNetworkCanvas
                entities={graphData?.entities ?? EMPTY_GRAPH_ENTITIES}
                relationships={
                  graphData?.relationships ?? EMPTY_GRAPH_RELATIONSHIPS
                }
                selectedEntityId={selectedEntityId}
                onSelectEntity={handleSelectEntityFromCanvas}
                isLoading={isGraphLoading && !graphData}
                className="h-full w-full"
              />
            </div>
          )}

          {/* CARDS MODE: Classic Grid */}
          {viewMode === 'cards' && (
            <Card className="flex h-full min-h-0 flex-col gap-4 py-4">
              <CardHeader className="px-4 pb-0">
                <CardTitle className="text-base">
                  {t('knowledge.browserTitle', 'Knowledge Browser')}
                </CardTitle>
                <CardDescription>
                  {t(
                    'knowledge.browserDescription',
                    'Find chunks first, then inspect their evidence and graph neighborhood.',
                  )}
                </CardDescription>
              </CardHeader>
              <CardContent className="flex min-h-0 flex-1 flex-col px-4 pt-2">
                {isInitialListLoading ? (
                  <div className="grid gap-3 lg:grid-cols-2 2xl:grid-cols-3">
                    {Array.from({ length: 6 }).map((_, index) => (
                      <div
                        key={index}
                        className="space-y-2 rounded-xl border p-3"
                      >
                        <Skeleton className="h-4 w-24" />
                        <Skeleton className="h-3 w-full" />
                        <Skeleton className="h-3 w-4/5" />
                      </div>
                    ))}
                  </div>
                ) : items.length === 0 ? (
                  <div className="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
                    {t(
                      'knowledge.emptyState',
                      'No knowledge entries match the current filters.',
                    )}
                  </div>
                ) : (
                  <div className="min-h-0 flex-1">
                    <VirtuosoGrid
                      className="h-full"
                      style={{ height: '100%' }}
                      data={items}
                      components={knowledgeGridComponents}
                      computeItemKey={(_index, item) => item.id}
                      context={gridContext}
                      increaseViewportBy={{ top: 160, bottom: 240 }}
                      itemContent={renderKnowledgeCard}
                      scrollSeekConfiguration={{
                        enter: (velocity) => Math.abs(velocity) > 400,
                        exit: (velocity) => Math.abs(velocity) < 80,
                      }}
                    />
                  </div>
                )}
              </CardContent>
            </Card>
          )}

          {/* Docked Slide-over Knowledge Inspector Sheet */}
          <KnowledgeInspectorSheet
            isOpen={inspectTarget !== null}
            onClose={handleCloseInspector}
            target={inspectTarget}
            chunkItem={selectedItem}
            chunkDetail={detail}
            isChunkLoading={isDetailLoading}
            isDeleteConfirming={isDeleteConfirming}
            isDeleting={isDeleting}
            onCancelDelete={cancelDelete}
            onRequestDelete={
              isDeleteConfirming
                ? () => void deleteSelectedItem()
                : requestDelete
            }
            entity={selectedEntity}
            allEntities={
              graphData?.entities ??
              detail?.entities ??
              EMPTY_GRAPH_ENTITIES
            }
            allRelationships={
              graphData?.relationships ??
              detail?.relationships ??
              EMPTY_GRAPH_RELATIONSHIPS
            }
            allChunks={items}
            onSelectEntity={handleSelectEntityFromInspector}
            onSelectChunk={handleSelectChunk}
          />
        </div>
      </div>
    </div>
  );
}
