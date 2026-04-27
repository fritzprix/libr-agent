import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui';
import { ScrollArea } from '@/components/ui/scroll-area';
import type { KnowledgeChunkDetail } from '@/lib/backend/knowledge';
import { KnowledgeGraphPreview } from './KnowledgeGraphPreview';

interface KnowledgeDetailGraphTabProps {
  detail: KnowledgeChunkDetail;
}

export function KnowledgeDetailGraphTab({
  detail,
}: KnowledgeDetailGraphTabProps) {
  const { t } = useTranslation('common');
  const entityNameById = useMemo(
    () => new Map(detail.entities.map((entity) => [entity.id, entity.name])),
    [detail.entities],
  );

  return (
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
                  <div key={relationship.id} className="rounded-lg border p-3">
                    <div className="font-medium">
                      {relationship.relationType}
                    </div>
                    <div className="mt-1 text-xs text-muted-foreground">
                      {entityNameById.get(relationship.sourceEntityId) ??
                        relationship.sourceEntityId}{' '}
                      -&gt;{' '}
                      {entityNameById.get(relationship.targetEntityId) ??
                        relationship.targetEntityId}
                    </div>
                  </div>
                ))
              )}
            </div>
          </ScrollArea>
        </CardContent>
      </Card>
    </div>
  );
}
