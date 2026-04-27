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
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import { DeleteKnowledgeDialog } from './components/DeleteKnowledgeDialog';
import { KnowledgeDetailDialog } from './components/KnowledgeDetailDialog';
import { KnowledgeListItemCard } from './components/KnowledgeListItemCard';
import { useKnowledgeBrowser } from './hooks/useKnowledgeBrowser';

export default function KnowledgePage() {
  const { t } = useTranslation('common');
  const {
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
  } = useKnowledgeBrowser();

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
            <RefreshCw className="h-4 w-4" />
            {t('knowledge.refresh', 'Refresh')}
          </Button>
        </div>

        <div className="min-h-0 flex-1">
          <Card className="min-h-0 gap-4 py-4">
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
                  {assistants.map((assistantId) => (
                    <SelectItem key={assistantId} value={assistantId}>
                      {assistantId}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>

              <ScrollArea className="min-h-0 flex-1">
                <div className="space-y-4 pr-3">
                  <div className="grid gap-3 lg:grid-cols-2 2xl:grid-cols-3">
                    {isListLoading ? (
                      Array.from({ length: 6 }).map((_, index) => (
                        <div
                          key={index}
                          className="space-y-2 rounded-xl border p-3"
                        >
                          <Skeleton className="h-4 w-24" />
                          <Skeleton className="h-3 w-full" />
                          <Skeleton className="h-3 w-4/5" />
                        </div>
                      ))
                    ) : items.length === 0 ? (
                      <div className="rounded-xl border border-dashed p-6 text-center text-sm text-muted-foreground">
                        {t(
                          'knowledge.emptyState',
                          'No knowledge entries match the current filters.',
                        )}
                      </div>
                    ) : (
                      items.map((item) => (
                        <KnowledgeListItemCard
                          key={item.id}
                          item={item}
                          isActive={item.id === selectedId}
                          onSelect={selectItem}
                        />
                      ))
                    )}
                  </div>

                  {!isListLoading && items.length > 0 ? (
                    <div className="flex flex-col items-center gap-3 py-2">
                      {hasMoreItems ? (
                        <Button
                          type="button"
                          variant="outline"
                          onClick={() => void loadMore()}
                          disabled={isLoadingMore}
                          className="min-w-36 gap-2"
                        >
                          {isLoadingMore ? (
                            <Loader2 className="h-4 w-4 animate-spin" />
                          ) : null}
                          {t('knowledge.loadMore', 'Load more')}
                        </Button>
                      ) : (
                        <p className="text-xs text-muted-foreground">
                          {t(
                            'knowledge.endOfResults',
                            'You have reached the end of the current results.',
                          )}
                        </p>
                      )}
                    </div>
                  ) : null}
                </div>
              </ScrollArea>
            </CardContent>
          </Card>
        </div>
      </div>
      <KnowledgeDetailDialog
        open={isDetailOpen}
        detail={detail}
        entityNameById={entityNameById}
        isDeleting={isDeleting}
        isDetailLoading={isDetailLoading}
        onClose={closeDetail}
        onRequestDelete={requestDelete}
        selectedItem={selectedItem}
      />
      <DeleteKnowledgeDialog
        open={isDeleteDialogOpen}
        isDeleting={isDeleting}
        onOpenChange={(open) => !isDeleting && setIsDeleteDialogOpen(open)}
        onConfirm={() => void deleteSelectedItem()}
      />
    </div>
  );
}
