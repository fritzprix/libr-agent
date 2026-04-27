import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/ui';
import type { KnowledgeChunkDetail } from '@/lib/backend/knowledge';
import { layoutGraphNodes } from '../../knowledge-utils';

interface KnowledgeGraphPreviewProps {
  detail: KnowledgeChunkDetail;
}

export function KnowledgeGraphPreview({ detail }: KnowledgeGraphPreviewProps) {
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
