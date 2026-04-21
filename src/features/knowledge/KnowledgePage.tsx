import {
  memo,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useState,
  useTransition,
} from 'react';
import { useTranslation } from 'react-i18next';
import {
  Database,
  FileText,
  Loader2,
  Network,
  RefreshCw,
  Search,
  Trash2,
} from 'lucide-react';
import { toast } from 'sonner';
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Input,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import { getLogger } from '@/lib/logger';
import {
  deleteGlobalKnowledge,
  type KnowledgeChunkDetail,
  type KnowledgeChunkListItem,
  type KnowledgeGraphEntity,
} from '@/lib/backend/knowledge';
import { useKnowledgeList } from './hooks/useKnowledgeList';
import { useKnowledgeDetail } from './hooks/useKnowledgeDetail';

const logger = getLogger('KnowledgePage');
const KNOWLEDGE_PAGE_SIZE = 60;
const knowledgeDateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
});

function formatTimestamp(timestamp: number): string {
  return knowledgeDateFormatter.format(new Date(timestamp));
}

function getKnowledgeCardTitle(preview: string): string {
  const normalizedPreview = preview.replace(/\s+/g, ' ').trim();
  if (!normalizedPreview) {
    return 'Untitled knowledge entry';
  }

  const sentenceMatch = normalizedPreview.match(/^(.{1,96}?[.!?])(?:\s|$)/);
  if (sentenceMatch?.[1]) {
    return sentenceMatch[1];
  }

  if (normalizedPreview.length <= 96) {
    return normalizedPreview;
  }

  return `${normalizedPreview.slice(0, 93)}...`;
}

function layoutGraphNodes(entities: KnowledgeGraphEntity[]) {
  const centerX = 180;
  const centerY = 140;
  const primary = entities.filter((entity) => entity.isPrimary);
  const secondary = entities.filter((entity) => !entity.isPrimary);
  const positions = new Map<number, { x: number; y: number }>();

  if (primary.length === 1) {
    positions.set(primary[0].id, { x: centerX, y: centerY });
  } else {
    primary.forEach((entity, index) => {
      const angle = (Math.PI * 2 * index) / Math.max(primary.length, 1);
      positions.set(entity.id, {
        x: centerX + Math.cos(angle) * 68,
        y: centerY + Math.sin(angle) * 68,
      });
    });
  }

  secondary.forEach((entity, index) => {
    const angle = (Math.PI * 2 * index) / Math.max(secondary.length, 1);
    positions.set(entity.id, {
      x: centerX + Math.cos(angle) * 118,
      y: centerY + Math.sin(angle) * 118,
    });
  });

  return positions;
}

function KnowledgeGraphPreview({ detail }: { detail: KnowledgeChunkDetail }) {
  const positions = useMemo(
    () => layoutGraphNodes(detail.entities),
    [detail.entities],
  );

  if (!detail.entities.length) {
    return (
      <div className="rounded-xl border border-dashed border-border/60 p-8 text-center text-sm text-muted-foreground">
        No graph data linked to this knowledge entry.
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="rounded-xl border bg-muted/20 p-3">
        <svg viewBox="0 0 360 280" className="h-[280px] w-full">
          {detail.relationships.map((relationship) => {
            const source = positions.get(relationship.sourceEntityId);
            const target = positions.get(relationship.targetEntityId);
            if (!source || !target) {
              return null;
            }

            const midX = (source.x + target.x) / 2;
            const midY = (source.y + target.y) / 2;

            return (
              <g key={relationship.id}>
                <line
                  x1={source.x}
                  y1={source.y}
                  x2={target.x}
                  y2={target.y}
                  stroke="currentColor"
                  strokeOpacity="0.25"
                  strokeWidth="1.5"
                  className="text-muted-foreground"
                />
                <text
                  x={midX}
                  y={midY}
                  textAnchor="middle"
                  className="fill-muted-foreground text-[9px]"
                >
                  {relationship.relationType}
                </text>
              </g>
            );
          })}

          {detail.entities.map((entity) => {
            const position = positions.get(entity.id);
            if (!position) {
              return null;
            }

            return (
              <g key={entity.id}>
                <circle
                  cx={position.x}
                  cy={position.y}
                  r={entity.isPrimary ? 20 : 16}
                  className={
                    entity.isPrimary
                      ? 'fill-primary/80 stroke-primary'
                      : 'fill-muted stroke-muted-foreground/40'
                  }
                  strokeWidth="1.5"
                />
                <text
                  x={position.x}
                  y={position.y + (entity.isPrimary ? 34 : 28)}
                  textAnchor="middle"
                  className="fill-foreground text-[10px] font-medium"
                >
                  {entity.name}
                </text>
              </g>
            );
          })}
        </svg>
      </div>

      <div className="flex flex-wrap gap-2 text-xs">
        <Badge variant="secondary" className="gap-1">
          <span className="inline-block h-2 w-2 rounded-full bg-primary" />
          Primary entity
        </Badge>
        <Badge variant="outline" className="gap-1">
          <span className="inline-block h-2 w-2 rounded-full bg-muted-foreground/60" />
          Related entity
        </Badge>
      </div>
    </div>
  );
}

interface KnowledgeListItemCardProps {
  item: KnowledgeChunkListItem;
  isActive: boolean;
  onSelect: (id: number) => void;
}

const KnowledgeListItemCard = memo(function KnowledgeListItemCard({
  item,
  isActive,
  onSelect,
}: KnowledgeListItemCardProps) {
  const title = getKnowledgeCardTitle(item.preview);
  const visibleTags = item.tags.slice(0, 2);
  const hiddenTagCount = Math.max(item.tags.length - visibleTags.length, 0);

  return (
    <button
      type="button"
      onClick={() => onSelect(item.id)}
      className={`w-full rounded-2xl border p-4 text-left shadow-sm transition-all [content-visibility:auto] ${
        isActive
          ? 'border-primary/70 bg-primary/5 shadow-primary/5'
          : 'border-border/60 bg-card/80 hover:border-border hover:bg-muted/30'
      }`}
      style={{ containIntrinsicSize: '220px' }}
    >
      <div className="min-w-0 space-y-3">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <Badge
            variant="secondary"
            className="max-w-full min-w-0 shrink basis-full justify-start truncate sm:basis-auto"
            title={item.assistantId}
          >
            {item.assistantId}
          </Badge>
          {item.source ? (
            <Badge
              variant="outline"
              className="max-w-full min-w-0 shrink basis-full justify-start truncate sm:max-w-[18rem] sm:basis-auto"
              title={item.source}
            >
              {item.source}
            </Badge>
          ) : null}
        </div>

        <h3
          className="line-clamp-2 text-sm font-semibold leading-5 text-foreground"
          title={title}
        >
          {title}
        </h3>

        <span className="block text-[11px] text-muted-foreground">
          {formatTimestamp(item.createdAt)}
        </span>
      </div>

      <div className="mt-4 rounded-xl border border-border/40 bg-muted/20 p-3">
        <div className="mb-2 text-[11px] font-medium uppercase tracking-[0.08em] text-muted-foreground">
          Excerpt
        </div>
        <p className="line-clamp-4 text-sm leading-6 text-foreground/90">
          {item.preview}
        </p>
      </div>

      <div className="mt-4 flex min-w-0 flex-wrap gap-2">
        {visibleTags.map((tag) => (
          <Badge
            key={tag}
            variant="outline"
            className="max-w-full min-w-0 justify-start truncate"
            title={tag}
          >
            #{tag}
          </Badge>
        ))}
        {hiddenTagCount > 0 ? (
          <Badge variant="outline" className="justify-start">
            +{hiddenTagCount}
          </Badge>
        ) : null}
      </div>
    </button>
  );
});

interface KnowledgeDetailDialogProps {
  open: boolean;
  detail: KnowledgeChunkDetail | null;
  entityNameById: Map<number, string>;
  isDeleting: boolean;
  isDetailLoading: boolean;
  onClose: () => void;
  onRequestDelete: () => void;
  selectedItem: KnowledgeChunkListItem | null;
}

const KnowledgeDetailDialog = memo(function KnowledgeDetailDialog({
  open,
  detail,
  entityNameById,
  isDeleting,
  isDetailLoading,
  onClose,
  onRequestDelete,
  selectedItem,
}: KnowledgeDetailDialogProps) {
  const { t } = useTranslation('common');

  if (!open) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent
        showCloseButton={false}
        className="grid h-[92vh] w-[calc(100vw-1.5rem)] max-w-[calc(100vw-1.5rem)] sm:!max-w-[min(1500px,calc(100vw-1.5rem))] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden border border-border/50 bg-background p-0 shadow-[0_28px_80px_-36px_rgba(0,0,0,0.45)]"
      >
        <DialogHeader className="border-b border-border/40 px-6 py-5 text-left">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <DialogTitle className="text-base">
                {t('knowledge.detailTitle', 'Knowledge Detail')}
              </DialogTitle>
              <DialogDescription>
                {t(
                  'knowledge.detailDescription',
                  'Inspect the selected entry, its evidence, and its local graph.',
                )}
              </DialogDescription>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button type="button" variant="outline" onClick={onClose}>
                {t('knowledge.close', 'Close')}
              </Button>
              <Button
                type="button"
                variant="destructive"
                onClick={onRequestDelete}
                disabled={!selectedItem || isDeleting}
                className="gap-2"
              >
                {isDeleting ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Trash2 className="h-4 w-4" />
                )}
                {t('knowledge.delete', 'Delete')}
              </Button>
            </div>
          </div>
        </DialogHeader>

        <div className="min-h-0 overflow-hidden px-6 py-5">
          {isDetailLoading || !detail ? (
            <div className="space-y-4">
              <Skeleton className="h-6 w-40" />
              <Skeleton className="h-28 w-full" />
              <Skeleton className="h-48 w-full" />
            </div>
          ) : (
            <Tabs
              defaultValue="overview"
              className="flex h-full flex-col gap-4"
            >
              <TabsList className="grid w-full max-w-md grid-cols-2">
                <TabsTrigger value="overview" className="gap-2">
                  <FileText className="h-4 w-4" />
                  {t('knowledge.tabs.overview', 'Overview')}
                </TabsTrigger>
                <TabsTrigger value="graph" className="gap-2">
                  <Network className="h-4 w-4" />
                  {t('knowledge.tabs.graph', 'Graph')}
                </TabsTrigger>
              </TabsList>

              <TabsContent value="overview" className="min-h-0 flex-1">
                <div className="grid h-full gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
                  <Card className="min-h-0 py-4">
                    <CardHeader className="px-4">
                      <div className="flex flex-wrap gap-2">
                        <Badge variant="secondary">{detail.assistantId}</Badge>
                        {detail.source ? (
                          <Badge variant="outline">{detail.source}</Badge>
                        ) : null}
                        {detail.tags.map((tag) => (
                          <Badge key={tag} variant="outline">
                            #{tag}
                          </Badge>
                        ))}
                      </div>
                    </CardHeader>
                    <CardContent className="min-h-0 px-4">
                      <ScrollArea className="h-[420px] pr-4">
                        <div className="whitespace-pre-wrap break-words text-sm leading-6 text-foreground">
                          {detail.content}
                        </div>
                      </ScrollArea>
                    </CardContent>
                  </Card>

                  <div className="flex min-h-0 flex-col gap-4">
                    <Card className="py-4">
                      <CardHeader className="px-4">
                        <CardTitle className="text-sm">
                          {t('knowledge.metadataTitle', 'Metadata')}
                        </CardTitle>
                      </CardHeader>
                      <CardContent className="space-y-2 px-4 text-sm">
                        <div>
                          <span className="text-muted-foreground">
                            {t('knowledge.fields.createdAt', 'Created')}
                          </span>
                          <div>{formatTimestamp(detail.createdAt)}</div>
                        </div>
                        <div>
                          <span className="text-muted-foreground">
                            {t(
                              'knowledge.fields.primaryEntities',
                              'Primary entities',
                            )}
                          </span>
                          <div>{detail.primaryEntityIds.length}</div>
                        </div>
                        <div>
                          <span className="text-muted-foreground">
                            {t(
                              'knowledge.fields.relationships',
                              'Relationships',
                            )}
                          </span>
                          <div>{detail.relationships.length}</div>
                        </div>
                      </CardContent>
                    </Card>

                    <Card className="min-h-0 flex-1 py-4">
                      <CardHeader className="px-4">
                        <CardTitle className="text-sm">
                          {t('knowledge.entitiesTitle', 'Entities')}
                        </CardTitle>
                      </CardHeader>
                      <CardContent className="min-h-0 px-4">
                        <ScrollArea className="h-[250px] pr-3">
                          <div className="space-y-2">
                            {detail.entities.length === 0 ? (
                              <p className="text-sm text-muted-foreground">
                                {t(
                                  'knowledge.noEntities',
                                  'No linked entities for this entry.',
                                )}
                              </p>
                            ) : (
                              detail.entities.map((entity) => (
                                <div
                                  key={entity.id}
                                  className="rounded-lg border p-3 text-sm"
                                >
                                  <div className="flex items-center gap-2">
                                    <span className="font-medium">
                                      {entity.name}
                                    </span>
                                    {entity.isPrimary ? (
                                      <Badge variant="secondary">
                                        {t(
                                          'knowledge.primaryEntity',
                                          'Primary',
                                        )}
                                      </Badge>
                                    ) : null}
                                  </div>
                                  {entity.entityType ? (
                                    <div className="mt-1 text-xs text-muted-foreground">
                                      {entity.entityType}
                                    </div>
                                  ) : null}
                                  {entity.description ? (
                                    <p className="mt-2 text-muted-foreground">
                                      {entity.description}
                                    </p>
                                  ) : null}
                                </div>
                              ))
                            )}
                          </div>
                        </ScrollArea>
                      </CardContent>
                    </Card>
                  </div>
                </div>
              </TabsContent>

              <TabsContent value="graph" className="min-h-0 flex-1">
                <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_320px]">
                  <KnowledgeGraphPreview detail={detail} />

                  <Card className="py-4">
                    <CardHeader className="px-4">
                      <CardTitle className="text-sm">
                        {t('knowledge.relationshipListTitle', 'Relationships')}
                      </CardTitle>
                    </CardHeader>
                    <CardContent className="px-4">
                      <ScrollArea className="h-[320px] pr-3">
                        <div className="space-y-2 text-sm">
                          {detail.relationships.length === 0 ? (
                            <p className="text-muted-foreground">
                              {t(
                                'knowledge.noRelationships',
                                'No relationships linked to this entry.',
                              )}
                            </p>
                          ) : (
                            detail.relationships.map((relationship) => (
                              <div
                                key={relationship.id}
                                className="rounded-lg border p-3"
                              >
                                <div className="font-medium">
                                  {relationship.relationType}
                                </div>
                                <div className="mt-1 text-xs text-muted-foreground">
                                  {entityNameById.get(
                                    relationship.sourceEntityId,
                                  ) ?? relationship.sourceEntityId}{' '}
                                  -&gt;{' '}
                                  {entityNameById.get(
                                    relationship.targetEntityId,
                                  ) ?? relationship.targetEntityId}
                                </div>
                              </div>
                            ))
                          )}
                        </div>
                      </ScrollArea>
                    </CardContent>
                  </Card>
                </div>
              </TabsContent>
            </Tabs>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
});

export default function KnowledgePage() {
  const { t } = useTranslation('common');
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const [assistantFilter, setAssistantFilter] = useState('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);
  const deferredQuery = useDeferredValue(query);
  const [, startSelectionTransition] = useTransition();

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setDebouncedQuery(deferredQuery.trim());
    }, 250);

    return () => window.clearTimeout(timeout);
  }, [deferredQuery]);

  const {
    items,
    assistants,
    nextCursor,
    isListLoading,
    isLoadingMore,
    loadMore: handleLoadMore,
    mutateList,
  } = useKnowledgeList(debouncedQuery, assistantFilter, KNOWLEDGE_PAGE_SIZE);

  const { detail, isDetailLoading } = useKnowledgeDetail(selectedId);

  const selectedItem = useMemo(
    () => items.find((item) => item.id === selectedId) ?? null,
    [items, selectedId],
  );

  // Weaver Pattern: Adjusting State During Render
  // Deselect if the item goes away from the list to prevent accessing a non-existent item detail.
  // We check that isListLoading is false to avoid deselecting while the list is momentarily empty during the very first load or un-cached filter change.
  if (
    selectedId !== null &&
    !isListLoading &&
    !items.some((item) => item.id === selectedId)
  ) {
    setSelectedId(null);
  }
  const isDetailOpen = selectedItem !== null;
  const hasMoreItems = nextCursor !== null;
  const entityNameById = useMemo(
    () =>
      new Map(detail?.entities.map((entity) => [entity.id, entity.name]) ?? []),
    [detail?.entities],
  );

  const handleRefresh = useCallback(() => {
    void mutateList();
  }, [mutateList]);

  const handleCloseDetail = useCallback(() => {
    setIsDeleteDialogOpen(false);
    setSelectedId(null);
  }, []);

  const handleSelectItem = useCallback((id: number) => {
    startSelectionTransition(() => {
      setSelectedId(id);
    });
  }, []);

  const handleRequestDelete = useCallback(() => {
    if (!selectedItem || isDeleting) {
      return;
    }
    setIsDeleteDialogOpen(true);
  }, [isDeleting, selectedItem]);

  const handleDelete = useCallback(async () => {
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
      setSelectedId(null);
      setIsDeleteDialogOpen(false);
      void mutateList();
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
  }, [isDeleting, mutateList, selectedItem, t]);

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
            onClick={handleRefresh}
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
                          onSelect={handleSelectItem}
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
                          onClick={() => void handleLoadMore()}
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
        onClose={handleCloseDetail}
        onRequestDelete={handleRequestDelete}
        selectedItem={selectedItem}
      />
      <AlertDialog
        open={isDeleteDialogOpen}
        onOpenChange={(open) => !isDeleting && setIsDeleteDialogOpen(open)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('knowledge.confirmDeleteTitle', 'Delete knowledge entry')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t(
                'knowledge.confirmDelete',
                'Delete this knowledge entry and clean up orphaned graph data?',
              )}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDeleting}>
              {t('knowledge.cancel', 'Cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              onClick={(event) => {
                event.preventDefault();
                void handleDelete();
              }}
              disabled={isDeleting}
              className="gap-2"
            >
              {isDeleting ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
              {t('knowledge.delete', 'Delete')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
