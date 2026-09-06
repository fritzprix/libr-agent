import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui';
import { Network, Sparkles } from 'lucide-react';
import type { SimulationNode } from './knowledge-graph-types';

export interface KnowledgeNodePeekCardProps {
  node: SimulationNode | null;
  position: { x: number; y: number } | null;
  containerBounds?: { width: number; height: number };
}

export function KnowledgeNodePeekCard({
  node,
  position,
  containerBounds,
}: KnowledgeNodePeekCardProps) {
  const { t } = useTranslation('common');

  const cardStyle = useMemo(() => {
    if (!position) return { display: 'none' };

    const cardWidth = 260;
    const cardHeight = 140;
    const offset = 16;

    let left = position.x + offset;
    let top = position.y + offset;

    if (containerBounds) {
      if (left + cardWidth > containerBounds.width - 12) {
        left = position.x - cardWidth - offset;
      }
      if (top + cardHeight > containerBounds.height - 12) {
        top = position.y - cardHeight - offset;
      }

      left = Math.max(
        12,
        Math.min(left, containerBounds.width - cardWidth - 12),
      );
      top = Math.max(
        12,
        Math.min(top, containerBounds.height - cardHeight - 12),
      );
    }

    return {
      left: `${left}px`,
      top: `${top}px`,
    };
  }, [position, containerBounds]);

  if (!node || !position) {
    return null;
  }

  return (
    <div
      style={cardStyle}
      className="pointer-events-none absolute z-30 w-64 select-none rounded-xl border border-border/70 bg-background/85 p-3.5 shadow-2xl backdrop-blur-md transition-opacity duration-150 animate-in fade-in-50 zoom-in-95"
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            {node.isPrimary && (
              <Sparkles className="h-3.5 w-3.5 shrink-0 text-amber-500 dark:text-amber-400" />
            )}
            <h4 className="truncate text-sm font-semibold tracking-tight text-foreground">
              {node.name}
            </h4>
          </div>
          {node.entityType ? (
            <div className="mt-1">
              <Badge
                variant="outline"
                className="h-4.5 px-1.5 py-0 text-[10px] font-medium uppercase tracking-wider"
              >
                {node.entityType}
              </Badge>
            </div>
          ) : null}
        </div>

        <div className="flex shrink-0 items-center gap-1 rounded-md bg-muted/60 px-1.5 py-0.5 text-[11px] font-medium text-muted-foreground">
          <Network className="h-3 w-3" />
          <span>{node.connectionCount}</span>
        </div>
      </div>

      {node.description ? (
        <p className="mt-2 line-clamp-3 text-xs leading-relaxed text-muted-foreground">
          {node.description}
        </p>
      ) : (
        <p className="mt-2 text-xs italic text-muted-foreground/70">
          {t('knowledge.graph.noDescription', 'No description available.')}
        </p>
      )}

      <div className="mt-2.5 flex items-center justify-between border-t border-border/50 pt-2 text-[10px] text-muted-foreground">
        <span>
          {t('knowledge.graph.clickToFocus', 'Click to focus neighborhood')}
        </span>
        {node.isPrimary && (
          <span className="font-medium text-primary">
            {t('knowledge.primary', 'Primary')}
          </span>
        )}
      </div>
    </div>
  );
}
