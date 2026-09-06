import { memo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import {
  AlertTriangle,
  ArrowDownRight,
  ArrowUpRight,
  Calendar,
  ExternalLink,
  FileText,
  Hash,
  Loader2,
  Network,
  Trash2,
  X,
} from 'lucide-react';
import { Badge, Button } from '@/components/ui';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';
import type {
  KnowledgeChunkDetail,
  KnowledgeChunkListItem,
  KnowledgeGraphEntity,
  KnowledgeGraphRelationship,
} from '@/lib/backend/knowledge';
import { formatTimestamp, getKnowledgeCardTitle } from '../knowledge-utils';
import { getNodeColor } from './graph/knowledge-graph-types';

export type InspectedTarget =
  | { type: 'chunk'; id: number }
  | { type: 'entity'; id: number };

export interface KnowledgeInspectorSheetProps {
  isOpen: boolean;
  onClose: () => void;
  target: InspectedTarget | null;
  // Chunk inspection props
  chunkItem?: KnowledgeChunkListItem | null;
  chunkDetail?: KnowledgeChunkDetail | null;
  isChunkLoading?: boolean;
  isDeleteConfirming?: boolean;
  isDeleting?: boolean;
  onCancelDelete?: () => void;
  onRequestDelete?: () => void;
  // Entity inspection props
  entity?: KnowledgeGraphEntity | null;
  allEntities?: KnowledgeGraphEntity[];
  allRelationships?: KnowledgeGraphRelationship[];
  allChunks?: KnowledgeChunkListItem[];
  // Interactive links navigation
  onSelectEntity?: (entityId: number) => void;
  onSelectChunk?: (chunkId: number) => void;
  className?: string;
}

export const KnowledgeInspectorSheet = memo(function KnowledgeInspectorSheet({
  isOpen,
  onClose,
  target,
  chunkItem,
  chunkDetail,
  isChunkLoading = false,
  isDeleteConfirming = false,
  isDeleting = false,
  onCancelDelete,
  onRequestDelete,
  entity,
  allEntities = [],
  allRelationships = [],
  allChunks = [],
  onSelectEntity,
  onSelectChunk,
  className,
}: KnowledgeInspectorSheetProps) {
  const { t } = useTranslation('common');

  // Listen for Escape key to close inspector
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  const isViewingChunk = target?.type === 'chunk';
  const isViewingEntity = target?.type === 'entity';

  // For Entity: compute connected relationships and linked chunks
  const connectedRelationships =
    isViewingEntity && entity
      ? allRelationships.filter(
          (rel) =>
            rel.sourceEntityId === entity.id ||
            rel.targetEntityId === entity.id,
        )
      : [];

  const entityLinkedChunks =
    isViewingEntity && entity
      ? allChunks.filter((chunk) => {
          const query = entity.name.toLowerCase();
          return (
            chunk.preview.toLowerCase().includes(query) ||
            chunk.tags.some((tag) => tag.toLowerCase().includes(query))
          );
        })
      : [];

  return (
    <aside
      aria-label={t('knowledge.inspectorTitle', 'Knowledge Inspector')}
      className={cn(
        'absolute right-0 top-0 bottom-0 z-30 flex flex-col',
        'w-full sm:w-[420px] lg:w-[460px]',
        'border-l border-border/60 bg-background/95 backdrop-blur-md shadow-2xl',
        'transition-transform duration-300 ease-in-out',
        isOpen ? 'translate-x-0' : 'translate-x-full pointer-events-none',
        className,
      )}
    >
      {/* Header */}
      <div className="flex shrink-0 items-start justify-between border-b border-border/60 px-5 py-4">
        <div className="min-w-0 flex-1 pr-3">
          {isViewingChunk && (
            <div className="space-y-1.5">
              <div className="flex items-center gap-2">
                <FileText className="h-4 w-4 shrink-0 text-primary" />
                <span className="truncate text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  {t('knowledge.chunkEntry', 'Knowledge Chunk')}
                </span>
                <span className="font-mono text-xs text-muted-foreground/80">
                  #{chunkItem?.id ?? chunkDetail?.id ?? target?.id}
                </span>
              </div>
              <h2
                className="truncate text-base font-semibold text-foreground"
                title={
                  chunkDetail
                    ? getKnowledgeCardTitle(
                        chunkDetail.content,
                        'Knowledge Chunk',
                      )
                    : chunkItem?.preview
                }
              >
                {chunkDetail
                  ? getKnowledgeCardTitle(
                      chunkDetail.content,
                      'Knowledge Chunk',
                    )
                  : chunkItem
                    ? getKnowledgeCardTitle(
                        chunkItem.preview,
                        'Knowledge Chunk',
                      )
                    : t('knowledge.chunkDetails', 'Chunk Details')}
              </h2>
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                {(chunkDetail?.assistantId || chunkItem?.assistantId) && (
                  <Badge variant="secondary" className="text-xs">
                    {chunkDetail?.assistantId ?? chunkItem?.assistantId}
                  </Badge>
                )}
                {(chunkDetail?.source || chunkItem?.source) && (
                  <Badge
                    variant="outline"
                    className="max-w-[200px] truncate text-xs"
                  >
                    {chunkDetail?.source ?? chunkItem?.source}
                  </Badge>
                )}
                {(chunkDetail?.tags ?? chunkItem?.tags ?? []).map((tag) => (
                  <Badge
                    key={tag}
                    variant="outline"
                    className="text-xs font-normal"
                  >
                    #{tag}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {isViewingEntity && entity && (
            <div className="space-y-1.5">
              <div className="flex items-center gap-2">
                <Network className="h-4 w-4 shrink-0 text-primary" />
                <span className="truncate text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  {t('knowledge.graphEntity', 'Knowledge Entity')}
                </span>
                <span className="font-mono text-xs text-muted-foreground/80">
                  #{entity.id}
                </span>
              </div>
              <h2 className="truncate text-base font-semibold text-foreground">
                {entity.name}
              </h2>
              <div className="flex flex-wrap items-center gap-1.5 pt-1">
                {entity.isPrimary && (
                  <Badge variant="default" className="bg-primary/90 text-xs">
                    {t('knowledge.primaryEntity', 'Primary')}
                  </Badge>
                )}
                {entity.entityType && (
                  <Badge
                    variant="outline"
                    className="text-xs font-medium"
                    style={{
                      borderColor: getNodeColor(
                        entity.entityType,
                        entity.isPrimary,
                        true,
                      ),
                      color: getNodeColor(
                        entity.entityType,
                        entity.isPrimary,
                        true,
                      ),
                    }}
                  >
                    {entity.entityType}
                  </Badge>
                )}
                {entity.assistantId && (
                  <Badge variant="secondary" className="text-xs">
                    {entity.assistantId}
                  </Badge>
                )}
              </div>
            </div>
          )}

          {!isViewingChunk && !isViewingEntity && (
            <h2 className="text-base font-semibold text-foreground">
              {t('knowledge.inspectorTitle', 'Knowledge Inspector')}
            </h2>
          )}
        </div>

        <Button
          type="button"
          variant="ghost"
          size="icon"
          onClick={onClose}
          className="h-8 w-8 shrink-0 rounded-lg text-muted-foreground hover:text-foreground"
          aria-label={t('common.close', 'Close')}
        >
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Main Body */}
      <ScrollArea className="min-h-0 flex-1 px-5 py-4">
        {/* VIEWING A CHUNK */}
        {isViewingChunk && (
          <div className="space-y-6">
            {isChunkLoading || !chunkDetail ? (
              <div className="space-y-3 py-2">
                <Skeleton className="h-4 w-28" />
                <Skeleton className="h-24 w-full rounded-xl" />
                <Skeleton className="h-16 w-full rounded-xl" />
              </div>
            ) : (
              <>
                {/* Formatted Content */}
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-wider text-muted-foreground">
                    <Hash className="h-3.5 w-3.5" />
                    <span>{t('knowledge.fields.content', 'Content')}</span>
                  </div>
                  <div className="rounded-xl border border-border/50 bg-muted/20 p-4">
                    <div className="prose prose-sm dark:prose-invert max-w-none break-words leading-relaxed">
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {chunkDetail.content}
                      </ReactMarkdown>
                    </div>
                  </div>
                </div>

                {/* Metadata */}
                <div className="rounded-xl border border-border/50 bg-muted/10 p-3 text-xs">
                  <div className="grid grid-cols-2 gap-2">
                    <div className="flex items-center gap-1.5 text-muted-foreground">
                      <Calendar className="h-3.5 w-3.5" />
                      <span>{formatTimestamp(chunkDetail.createdAt)}</span>
                    </div>
                    {chunkDetail.source ? (
                      <div
                        className="flex items-center gap-1.5 truncate text-muted-foreground"
                        title={chunkDetail.source}
                      >
                        <ExternalLink className="h-3.5 w-3.5 shrink-0" />
                        <span className="truncate">{chunkDetail.source}</span>
                      </div>
                    ) : null}
                  </div>
                </div>

                {/* Linked Entities */}
                <div className="space-y-2.5">
                  <div className="flex items-center justify-between text-xs font-medium uppercase tracking-wider text-muted-foreground">
                    <span>
                      {t('knowledge.linkedEntities', 'Linked Entities')} (
                      {chunkDetail.entities.length})
                    </span>
                  </div>

                  {chunkDetail.entities.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {t(
                        'knowledge.noEntities',
                        'No linked entities for this entry.',
                      )}
                    </p>
                  ) : (
                    <div className="flex flex-wrap gap-1.5">
                      {chunkDetail.entities.map((ent) => (
                        <button
                          key={ent.id}
                          type="button"
                          onClick={() => onSelectEntity?.(ent.id)}
                          className={cn(
                            'group flex items-center gap-1.5 rounded-lg border border-border/60 bg-card px-2.5 py-1 text-xs text-foreground transition-colors hover:border-primary/50 hover:bg-primary/5',
                            ent.isPrimary &&
                              'border-primary/40 bg-primary/5 font-medium',
                          )}
                        >
                          <span className="truncate group-hover:text-primary">
                            {ent.name}
                          </span>
                          {ent.entityType && (
                            <span className="text-[10px] text-muted-foreground">
                              ({ent.entityType})
                            </span>
                          )}
                        </button>
                      ))}
                    </div>
                  )}
                </div>

                {/* Linked Relationships */}
                <div className="space-y-2.5">
                  <div className="flex items-center justify-between text-xs font-medium uppercase tracking-wider text-muted-foreground">
                    <span>
                      {t('knowledge.linkedRelationships', 'Relationships')} (
                      {chunkDetail.relationships.length})
                    </span>
                  </div>

                  {chunkDetail.relationships.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {t(
                        'knowledge.noRelationships',
                        'No relationships connected to this entry.',
                      )}
                    </p>
                  ) : (
                    <div className="space-y-1.5">
                      {chunkDetail.relationships.map((rel) => {
                        const source = chunkDetail.entities.find(
                          (e) => e.id === rel.sourceEntityId,
                        );
                        const target = chunkDetail.entities.find(
                          (e) => e.id === rel.targetEntityId,
                        );
                        return (
                          <div
                            key={rel.id}
                            className="flex items-center gap-1.5 rounded-lg border border-border/40 bg-muted/20 px-2.5 py-1.5 text-xs"
                          >
                            <button
                              type="button"
                              onClick={() =>
                                onSelectEntity?.(rel.sourceEntityId)
                              }
                              className="font-medium text-foreground hover:text-primary hover:underline"
                            >
                              {source?.name ?? `#${rel.sourceEntityId}`}
                            </button>
                            <span className="font-mono text-[10px] text-muted-foreground">
                              —[{rel.relationType}]→
                            </span>
                            <button
                              type="button"
                              onClick={() =>
                                onSelectEntity?.(rel.targetEntityId)
                              }
                              className="font-medium text-foreground hover:text-primary hover:underline"
                            >
                              {target?.name ?? `#${rel.targetEntityId}`}
                            </button>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              </>
            )}
          </div>
        )}

        {/* VIEWING AN ENTITY */}
        {isViewingEntity && entity && (
          <div className="space-y-6">
            {/* Description */}
            <div className="space-y-2">
              <div className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                {t('knowledge.fields.description', 'Description')}
              </div>
              {entity.description ? (
                <div className="rounded-xl border border-border/50 bg-muted/20 p-3 text-sm leading-relaxed text-foreground/90">
                  {entity.description}
                </div>
              ) : (
                <p className="text-xs italic text-muted-foreground">
                  {t(
                    'knowledge.noDescription',
                    'No description provided for this entity.',
                  )}
                </p>
              )}
            </div>

            {/* Connected Relationships */}
            <div className="space-y-2.5">
              <div className="flex items-center justify-between text-xs font-medium uppercase tracking-wider text-muted-foreground">
                <span>
                  {t(
                    'knowledge.connectedRelationships',
                    'Connected Relationships',
                  )}{' '}
                  ({connectedRelationships.length})
                </span>
              </div>

              {connectedRelationships.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t(
                    'knowledge.noConnectedRelationships',
                    'No relationships connected to this entity.',
                  )}
                </p>
              ) : (
                <div className="space-y-1.5">
                  {connectedRelationships.map((rel) => {
                    const isOutgoing = rel.sourceEntityId === entity.id;
                    const partnerId = isOutgoing
                      ? rel.targetEntityId
                      : rel.sourceEntityId;
                    const partner = allEntities.find((e) => e.id === partnerId);

                    return (
                      <button
                        key={rel.id}
                        type="button"
                        onClick={() => onSelectEntity?.(partnerId)}
                        className="group flex w-full items-center justify-between gap-2 rounded-lg border border-border/50 bg-card p-2 text-left text-xs transition-colors hover:border-primary/50 hover:bg-primary/5"
                      >
                        <div className="flex min-w-0 items-center gap-2">
                          {isOutgoing ? (
                            <ArrowUpRight className="h-3.5 w-3.5 shrink-0 text-emerald-500" />
                          ) : (
                            <ArrowDownRight className="h-3.5 w-3.5 shrink-0 text-amber-500" />
                          )}
                          <Badge
                            variant="outline"
                            className="font-mono text-[10px] text-muted-foreground group-hover:border-primary/40 group-hover:text-primary"
                          >
                            {rel.relationType}
                          </Badge>
                          <span className="truncate font-medium text-foreground group-hover:text-primary">
                            {partner?.name ?? `Entity #${partnerId}`}
                          </span>
                        </div>
                        {partner?.entityType && (
                          <span className="shrink-0 text-[10px] text-muted-foreground">
                            {partner.entityType}
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
              )}
            </div>

            {/* Linked Chunks */}
            <div className="space-y-2.5">
              <div className="flex items-center justify-between text-xs font-medium uppercase tracking-wider text-muted-foreground">
                <span>
                  {t('knowledge.linkedChunks', 'Linked Chunks')} (
                  {entityLinkedChunks.length})
                </span>
              </div>

              {entityLinkedChunks.length === 0 ? (
                <p className="text-xs text-muted-foreground">
                  {t(
                    'knowledge.noLinkedChunks',
                    'No chunks referencing this entity found in current results.',
                  )}
                </p>
              ) : (
                <div className="space-y-2">
                  {entityLinkedChunks.map((chunk) => (
                    <button
                      key={chunk.id}
                      type="button"
                      onClick={() => onSelectChunk?.(chunk.id)}
                      className="group flex w-full flex-col gap-1.5 rounded-xl border border-border/60 bg-card p-3 text-left shadow-xs transition-colors hover:border-primary/50 hover:bg-primary/5"
                    >
                      <div className="flex items-center justify-between gap-2 text-[11px] text-muted-foreground">
                        <span className="font-mono">#{chunk.id}</span>
                        <span>{formatTimestamp(chunk.createdAt)}</span>
                      </div>
                      <p className="line-clamp-2 text-xs leading-relaxed text-foreground/90 group-hover:text-foreground">
                        {chunk.preview}
                      </p>
                      {chunk.tags.length > 0 && (
                        <div className="flex flex-wrap gap-1 pt-1">
                          {chunk.tags.slice(0, 3).map((tag) => (
                            <Badge
                              key={tag}
                              variant="secondary"
                              className="text-[10px] font-normal"
                            >
                              #{tag}
                            </Badge>
                          ))}
                        </div>
                      )}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </ScrollArea>

      {/* Footer (Delete action for chunks) */}
      {isViewingChunk && (
        <div className="shrink-0 border-t border-border/60 bg-muted/10 p-4">
          {isDeleteConfirming ? (
            <div className="space-y-3">
              <div className="flex items-start gap-2.5 rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-xs">
                <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
                <div className="space-y-0.5">
                  <p className="font-medium text-destructive">
                    {t(
                      'knowledge.confirmDeleteTitle',
                      'Delete knowledge entry',
                    )}
                  </p>
                  <p className="text-muted-foreground">
                    {t(
                      'knowledge.confirmDelete',
                      'Delete this knowledge entry and clean up orphaned graph data?',
                    )}
                  </p>
                </div>
              </div>
              <div className="flex items-center justify-end gap-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={onCancelDelete}
                  disabled={isDeleting}
                >
                  {t('knowledge.cancel', 'Cancel')}
                </Button>
                <Button
                  type="button"
                  variant="destructive"
                  size="sm"
                  onClick={onRequestDelete}
                  disabled={isDeleting}
                  className="gap-1.5"
                >
                  {isDeleting ? (
                    <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Trash2 className="h-3.5 w-3.5" />
                  )}
                  {t('knowledge.confirmDeleteAction', 'Delete permanently')}
                </Button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-end">
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={onRequestDelete}
                disabled={isDeleting}
                className="gap-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
              >
                <Trash2 className="h-3.5 w-3.5" />
                {t('knowledge.delete', 'Delete')}
              </Button>
            </div>
          )}
        </div>
      )}
    </aside>
  );
});
