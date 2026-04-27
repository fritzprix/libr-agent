import { useTranslation } from 'react-i18next';
import {
  Badge,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from '@/components/ui';
import { ScrollArea } from '@/components/ui/scroll-area';
import type { KnowledgeChunkDetail } from '@/lib/backend/knowledge';
import { formatTimestamp } from '../../knowledge-utils';

interface KnowledgeDetailOverviewTabProps {
  detail: KnowledgeChunkDetail;
}

export function KnowledgeDetailOverviewTab({
  detail,
}: KnowledgeDetailOverviewTabProps) {
  const { t } = useTranslation('common');

  return (
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
                {t('knowledge.fields.primaryEntities', 'Primary entities')}
              </span>
              <div>{detail.primaryEntityIds.length}</div>
            </div>
            <div>
              <span className="text-muted-foreground">
                {t('knowledge.fields.relationships', 'Relationships')}
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
                        <span className="font-medium">{entity.name}</span>
                        {entity.isPrimary ? (
                          <Badge variant="secondary">
                            {t('knowledge.primaryEntity', 'Primary')}
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
  );
}
