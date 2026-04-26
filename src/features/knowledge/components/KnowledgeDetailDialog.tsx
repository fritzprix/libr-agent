import { memo, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { FileText, Loader2, Network, Trash2 } from 'lucide-react';
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import type {
  KnowledgeChunkDetail,
  KnowledgeChunkListItem,
} from '@/lib/backend/knowledge';
import { formatTimestamp, layoutGraphNodes } from '../knowledge-utils';

function KnowledgeGraphPreview({ detail }: { detail: KnowledgeChunkDetail }) {
  const { t } = useTranslation('common');
  const positions = useMemo(
    () => layoutGraphNodes(detail.entities),
    [detail.entities],
  );

  if (!detail.entities.length) {
    return (
      <div className="rounded-xl border border-dashed border-border/60 p-8 text-center text-sm text-muted-foreground">
        {t(
          'knowledge.graph.empty',
          'No graph data linked to this knowledge entry.',
        )}
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
          {t('knowledge.graph.primaryEntity', 'Primary entity')}
        </Badge>
        <Badge variant="outline" className="gap-1">
          <span className="inline-block h-2 w-2 rounded-full bg-muted-foreground/60" />
          {t('knowledge.graph.relatedEntity', 'Related entity')}
        </Badge>
      </div>
    </div>
  );
}

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

export const KnowledgeDetailDialog = memo(function KnowledgeDetailDialog({
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
        className="grid h-[92vh] w-[calc(100vw-1.5rem)] max-w-[calc(100vw-1.5rem)] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden border border-border/50 bg-background p-0 shadow-[0_28px_80px_-36px_rgba(0,0,0,0.45)] sm:!max-w-[min(1500px,calc(100vw-1.5rem))]"
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
