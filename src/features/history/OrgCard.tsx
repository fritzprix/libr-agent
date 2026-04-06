import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Building2, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Card,
  CardContent,
  CardDescription,
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
      <CardHeader className="gap-4 border-b bg-muted/20">
        <div className="space-y-1">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
            {t('orgHistory.cardLabel', 'Explicit Org')}
          </p>
          <CardTitle className="flex items-center gap-2 text-xl leading-tight">
            <Building2 className="h-5 w-5 text-primary" />
            <span className="truncate">{org.orgName}</span>
          </CardTitle>
          <CardDescription className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span>
              {t('orgHistory.rootLabel', 'Root Session')}:{' '}
              <span className="font-medium text-foreground">
                {org.rootSession.name ?? org.orgRootSessionId}
              </span>
            </span>
            <span className="hidden sm:inline">•</span>
            <span className="truncate">ID {org.orgId}</span>
          </CardDescription>
        </div>
      </CardHeader>

      <CardContent className="space-y-5 pt-6">
        <OrgStatTiles
          memberCount={org.memberCount}
          busyCount={org.busyCount}
          updatedAt={org.updatedAt}
        />

        <div className="rounded-xl border bg-muted/15 p-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <div>
              <div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                {t('orgHistory.rootSection', 'Root Session')}
              </div>
              <div className="mt-1 truncate text-sm font-semibold">
                {org.rootSession.name ?? org.orgRootSessionId}
              </div>
            </div>
            <Badge
              variant="outline"
              className={cn('shrink-0', rootBadge.className)}
            >
              {t(
                `sessionHistory.status.${org.rootSession.status}`,
                rootBadge.label,
              )}
            </Badge>
          </div>

          <div className="grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
            <div className="rounded-lg bg-background/80 px-3 py-2">
              <span className="font-medium text-foreground">
                {t('orgHistory.rootId', 'Root ID')}
              </span>
              <div className="mt-1 truncate">{org.orgRootSessionId}</div>
            </div>
            <div className="rounded-lg bg-background/80 px-3 py-2">
              <span className="font-medium text-foreground">
                {t('orgHistory.lastUpdated', 'Last updated')}
              </span>
              <div className="mt-1 truncate" title={ts.tooltip}>
                {ts.display}
              </div>
            </div>
          </div>
        </div>

        <OrgLineageSnapshot
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
