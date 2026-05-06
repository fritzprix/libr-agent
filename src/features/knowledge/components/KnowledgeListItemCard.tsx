import { memo } from 'react';
import { Badge } from '@/components/ui';
import type { KnowledgeChunkListItem } from '@/lib/backend/knowledge';
import { formatTimestamp, getKnowledgeCardTitle } from '../knowledge-utils';

interface KnowledgeListItemCardProps {
  excerptLabel: string;
  item: KnowledgeChunkListItem;
  isActive: boolean;
  onSelect: (id: number) => void;
  untitledLabel: string;
}

export const KnowledgeListItemCard = memo(function KnowledgeListItemCard({
  excerptLabel,
  item,
  isActive,
  onSelect,
  untitledLabel,
}: KnowledgeListItemCardProps) {
  const title = getKnowledgeCardTitle(item.preview, untitledLabel);
  const visibleTags = item.tags.slice(0, 2);
  const hiddenTagCount = Math.max(item.tags.length - visibleTags.length, 0);

  return (
    <button
      type="button"
      onClick={() => onSelect(item.id)}
      className={`min-h-[220px] w-full rounded-2xl border p-4 text-left shadow-sm transition-all ${
        isActive
          ? 'border-primary/70 bg-primary/5 shadow-primary/5'
          : 'border-border/60 bg-card/80 hover:border-border hover:bg-muted/30'
      }`}
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
          {excerptLabel}
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
