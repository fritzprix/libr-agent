import { forwardRef, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Database, Loader2, RefreshCw, Search } from 'lucide-react';
import {
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
  VirtuosoGrid,
} from 'react-virtuoso';
import { cn } from '@/lib/utils';
import type { KnowledgeChunkListItem } from '@/lib/backend/knowledge';
import { KnowledgeDetailDialog } from './components/KnowledgeDetailDialog';
import { KnowledgeListItemCard } from './components/KnowledgeListItemCard';
import { useKnowledgeBrowser } from './hooks/useKnowledgeBrowser';

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
    hasMoreItems,
    isDeleteConfirming,
    isDeleting,
    isDetailLoading,
    isDetailOpen,
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
  } = useKnowledgeBrowser();

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

  const handleLoadMore = useCallback(() => {
    void loadMore();
  }, [loadMore]);

  const gridContext = useMemo<KnowledgeGridContext>(
    () => ({
      endOfResultsLabel,
      excerptLabel,
      hasMoreItems,
      isLoadingMore,
      loadMoreLabel,
      onLoadMore: handleLoadMore,
      onSelect: selectItem,
      selectedId,
      untitledLabel,
    }),
    [
      endOfResultsLabel,
      excerptLabel,
      hasMoreItems,
      isLoadingMore,
      loadMoreLabel,
      handleLoadMore,
      selectItem,
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
      <div className="mx-auto flex h-full max-w-7xl flex-col gap-6">
        <div className="flex flex-wrap items-start justify-between gap-4">
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

          <Button
            type="button"
            variant="outline"
            onClick={refresh}
            className="gap-2"
          >
            <RefreshCw
              className={cn('h-4 w-4', isListLoading && 'animate-spin')}
            />
            {t('knowledge.refresh', 'Refresh')}
          </Button>
        </div>

        <div className="min-h-0 flex-1">
          <Card className="flex h-full min-h-0 flex-col gap-4 py-4">
            <CardHeader className="px-4">
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
            <CardContent className="flex min-h-0 flex-1 flex-col gap-4 px-4">
              <div className="relative">
                <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder={t(
                    'knowledge.searchPlaceholder',
                    'Search content, tags, or source...',
                  )}
                  className="pl-9"
                />
              </div>

              <Select
                value={assistantFilter}
                onValueChange={setAssistantFilter}
              >
                <SelectTrigger>
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

              {isRefreshingList ? (
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  {t('knowledge.refreshingResults', 'Updating results...')}
                </div>
              ) : null}

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
        </div>
      </div>
      <KnowledgeDetailDialog
        open={isDetailOpen}
        detail={detail}
        isDeleteConfirming={isDeleteConfirming}
        isDeleting={isDeleting}
        isDetailLoading={isDetailLoading}
        onCancelDelete={cancelDelete}
        onClose={closeDetail}
        onRequestDelete={
          isDeleteConfirming ? () => void deleteSelectedItem() : requestDelete
        }
        selectedItem={selectedItem}
      />
    </div>
  );
}
