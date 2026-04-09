import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Building2, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { formatSessionTimestamp } from '@/lib/date-utils';
import { cn } from '@/lib/utils';
import type { OrgSummary } from './org-sessions';
import { getStatusBadgeConfig } from './org-status';
import { OrgStatTiles } from './OrgStatTiles';
import { OrgLineageSnapshot } from './OrgLineageSnapshot';

interface OrgCardProps {
  org: OrgSummary;
}

export function OrgCard({ org }: OrgCardProps) {
  const navigate = useNavigate();
  const { t } = useTranslation('common');

  const ts = formatSessionTimestamp(org.updatedAt);
  const rootBadge = getStatusBadgeConfig(org.rootSession.status);

  return (
    <Card className="overflow-hidden border-border/70 bg-card shadow-sm shadow-black/5 transition-shadow hover:shadow-md">
      <CardHeader className="gap-4 overflow-hidden border-b bg-muted/20">
        <div className="min-w-0 space-y-3">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
            {t('orgHistory.cardLabel', 'Explicit Org')}
          </p>
          <div className="min-w-0 space-y-2 overflow-hidden">
            <CardTitle className="flex min-w-0 max-w-full items-center gap-2 overflow-hidden pr-2 text-xl leading-tight">
              <Building2 className="h-5 w-5 shrink-0 text-primary" />
              <span className="truncate pr-2">{org.orgName}</span>
            </CardTitle>
            <div className="max-w-full overflow-hidden">
              <Badge
                variant="outline"
                className={cn(
                  'inline-flex max-w-full overflow-hidden align-top',
                  rootBadge.className,
                )}
              >
                <span className="truncate">
                  {t(
                    `sessionHistory.status.${org.rootSession.status}`,
                    rootBadge.label,
                  )}
                </span>
              </Badge>
            </div>
          </div>
          <div className="space-y-1 overflow-hidden pr-3 text-sm text-muted-foreground">
            <div
              className="max-w-full truncate pr-2"
              title={org.rootSession.name ?? org.orgRootSessionId}
            >
              {t('orgHistory.rootLabel', 'Root Session')}:{' '}
              <span className="font-medium text-foreground">
                {org.rootSession.name ?? org.orgRootSessionId}
              </span>
            </div>
            <div className="max-w-full truncate pr-2" title={ts.tooltip}>
              {t('orgHistory.updatedLabel', 'Updated')}:{' '}
              {ts.relative ?? ts.display}
            </div>
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-5 pt-6">
        <OrgStatTiles memberCount={org.memberCount} busyCount={org.busyCount} />

        <OrgLineageSnapshot
          rootSession={org.rootSession}
          members={org.members}
          orgRootSessionId={org.orgRootSessionId}
        />
      </CardContent>

      <CardFooter className="border-t bg-muted/10">
        <Button
          size="sm"
          className="ml-auto w-full sm:w-auto"
          onClick={() => navigate(`/agent/${org.orgRootSessionId}`)}
        >
          <Play className="mr-2 h-4 w-4" />
          {t('orgHistory.resumeRoot', 'Resume Root Session')}
        </Button>
      </CardFooter>
    </Card>
  );
}
