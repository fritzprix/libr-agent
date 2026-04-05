import { useCallback, useMemo } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import {
  Activity,
  Building2,
  Clock3,
  Play,
  RefreshCw,
  Users,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Card,
  CardContent,
  CardFooter,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { formatSessionTimestamp } from '@/lib/date-utils';
import { cn } from '@/lib/utils';
import { selectOrgSummaries } from './org-sessions';

function getStatusBadgeConfig(status: string) {
  switch (status) {
    case 'busy':
      return {
        label: 'Active',
        className: 'border-warning/30 bg-warning/10 text-warning-foreground',
      };
    case 'paused':
      return {
        label: 'Paused',
        className:
          'border-muted-foreground/20 bg-muted text-muted-foreground opacity-90',
      };
    case 'error':
      return {
        label: 'Error',
        className:
          'border-destructive/30 bg-destructive/10 text-destructive dark:text-destructive',
      };
    case 'idle':
    default:
      return {
        label: 'Idle',
        className: 'border-border bg-secondary text-secondary-foreground',
      };
  }
}

export default function Org() {
  const navigate = useNavigate();
  const { t } = useTranslation('common');
  const { sessions, isSessionsListLoading } = useAgentSessionListState();
  const { loadSessions } = useAgentSessionListActions();

  const orgs = useMemo(() => selectOrgSummaries(sessions), [sessions]);

  const handleResumeRoot = useCallback(
    (rootSessionId: string) => {
      navigate(`/agent/${rootSessionId}`);
    },
    [navigate],
  );

  if (isSessionsListLoading) {
    return (
      <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
        <div className="flex items-center justify-between">
          <div>
            <Skeleton className="h-8 w-40 mb-2" />
            <Skeleton className="h-4 w-96" />
          </div>
          <Skeleton className="h-9 w-24" />
        </div>
        <div className="grid gap-4 lg:grid-cols-2">
          {Array.from({ length: 3 }).map((_, index) => (
            <Card key={index} className="overflow-hidden">
              <CardHeader className="border-b bg-muted/20">
                <Skeleton className="h-6 w-32" />
                <Skeleton className="h-4 w-40" />
              </CardHeader>
              <CardContent className="space-y-4 pt-6">
                <Skeleton className="h-20 w-full rounded-xl" />
                <Skeleton className="h-28 w-full rounded-xl" />
                <Skeleton className="h-16 w-full" />
              </CardContent>
            </Card>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
      <div className="flex items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">
            {t('orgHistory.heading', 'Org View')}
          </h1>
          <p className="text-sm text-muted-foreground mt-1">
            {t(
              'orgHistory.description',
              'Explicit org-created lineages only. One-off delegated sessions stay in ordinary history.',
            )}
          </p>
        </div>
        <Button variant="outline" size="sm" onClick={loadSessions}>
          <RefreshCw className="mr-2 h-4 w-4" />
          {t('history.refresh', 'Refresh')}
        </Button>
      </div>

      {orgs.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border/70 bg-muted/10 py-20 text-center text-muted-foreground">
          <Building2 className="w-10 h-10 opacity-50" />
          <p className="text-base font-medium">
            {t('orgHistory.emptyState.title', 'No org lineages yet')}
          </p>
          <p className="text-sm max-w-xl">
            {t(
              'orgHistory.emptyState.subtitle',
              'Create an org explicitly, then spawn org members through the org-aware tools. Plain sub-agent lineage does not belong here.',
            )}
          </p>
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          {orgs.map((org) => (
            <Card
              key={org.orgId}
              className="overflow-hidden border-border/70 bg-card shadow-sm shadow-black/5 transition-shadow hover:shadow-md"
            >
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
                <div className="grid gap-3 sm:grid-cols-3">
                  <div className="rounded-xl border bg-background/80 p-3">
                    <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      <Users className="h-3.5 w-3.5" />
                      {t('orgHistory.membersLabel', 'Members')}
                    </div>
                    <div className="text-2xl font-semibold">
                      {org.memberCount}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {t('orgHistory.members', '{{count}} members', {
                        count: org.memberCount,
                      })}
                    </div>
                  </div>
                  <div className="rounded-xl border bg-background/80 p-3">
                    <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      <Activity className="h-3.5 w-3.5" />
                      {t('orgHistory.activeLabel', 'Active')}
                    </div>
                    <div className="text-2xl font-semibold">
                      {org.busyCount}
                    </div>
                    <div className="text-xs text-muted-foreground">
                      {t('orgHistory.busy', '{{count}} busy', {
                        count: org.busyCount,
                      })}
                    </div>
                  </div>
                  <div className="rounded-xl border bg-background/80 p-3">
                    <div className="mb-1 flex items-center gap-2 text-xs font-medium uppercase tracking-wide text-muted-foreground">
                      <Clock3 className="h-3.5 w-3.5" />
                      {t('orgHistory.updatedLabel', 'Updated')}
                    </div>
                    <div className="truncate text-sm font-semibold">
                      {formatSessionTimestamp(org.updatedAt).relative ??
                        formatSessionTimestamp(org.updatedAt).display}
                    </div>
                    <div
                      className="truncate text-xs text-muted-foreground"
                      title={formatSessionTimestamp(org.updatedAt).tooltip}
                    >
                      {formatSessionTimestamp(org.updatedAt).display}
                    </div>
                  </div>
                </div>

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
                      className={cn(
                        'shrink-0',
                        getStatusBadgeConfig(org.rootSession.status).className,
                      )}
                    >
                      {t(
                        `sessionHistory.status.${org.rootSession.status}`,
                        getStatusBadgeConfig(org.rootSession.status).label,
                      )}
                    </Badge>
                  </div>

                  <div className="grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
                    <div className="rounded-lg bg-background/80 px-3 py-2">
                      <span className="font-medium text-foreground">
                        {t('orgHistory.rootId', 'Root ID')}
                      </span>
                      <div className="mt-1 truncate">
                        {org.orgRootSessionId}
                      </div>
                    </div>
                    <div className="rounded-lg bg-background/80 px-3 py-2">
                      <span className="font-medium text-foreground">
                        {t('orgHistory.lastUpdated', 'Last updated')}
                      </span>
                      <div
                        className="mt-1 truncate"
                        title={formatSessionTimestamp(org.updatedAt).tooltip}
                      >
                        {formatSessionTimestamp(org.updatedAt).display}
                      </div>
                    </div>
                  </div>
                </div>

                <div className="space-y-3">
                  <div className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
                    {t('orgHistory.orgChart', 'Lineage Snapshot')}
                  </div>
                  <div className="space-y-2">
                    {org.members.slice(0, 5).map((member) => {
                      const depth = Math.min(member.depth ?? 0, 4);
                      const isRoot = member.id === org.orgRootSessionId;

                      return (
                        <div
                          key={member.id}
                          style={{ marginLeft: `${depth * 12}px` }}
                          className={cn(
                            'flex items-center justify-between gap-3 rounded-xl border p-3 transition-colors',
                            isRoot
                              ? 'border-primary/25 bg-primary/5'
                              : 'border-border/70 bg-background/80',
                          )}
                        >
                          <div className="flex min-w-0 items-start gap-3">
                            <span
                              className={cn(
                                'mt-1 h-2.5 w-2.5 shrink-0 rounded-full',
                                isRoot
                                  ? 'bg-primary'
                                  : member.status === 'busy'
                                    ? 'bg-warning'
                                    : member.status === 'error'
                                      ? 'bg-destructive'
                                      : 'bg-muted-foreground/40',
                              )}
                              aria-hidden="true"
                            />
                            <div className="min-w-0">
                              <div className="flex min-w-0 items-center gap-2">
                                <span className="truncate text-sm font-medium">
                                  {member.name ?? member.id}
                                </span>
                                {isRoot && (
                                  <Badge
                                    variant="secondary"
                                    className="shrink-0"
                                  >
                                    {t('orgHistory.rootBadge', 'Root')}
                                  </Badge>
                                )}
                              </div>
                              <div className="mt-1 truncate text-xs text-muted-foreground">
                                {t('orgHistory.depthLabel', 'Depth')}{' '}
                                {member.depth ?? 0} • {member.id}
                              </div>
                            </div>
                          </div>

                          <Badge
                            variant="outline"
                            className={cn(
                              'shrink-0',
                              getStatusBadgeConfig(member.status).className,
                            )}
                          >
                            {t(
                              `sessionHistory.status.${member.status}`,
                              getStatusBadgeConfig(member.status).label,
                            )}
                          </Badge>
                        </div>
                      );
                    })}
                    {org.members.length > 5 && (
                      <div className="rounded-xl border border-dashed border-border/70 px-4 py-3 text-sm text-muted-foreground">
                        {t(
                          'orgHistory.moreMembers',
                          '+{{count}} more members',
                          {
                            count: org.members.length - 5,
                          },
                        )}
                      </div>
                    )}
                  </div>
                </div>
              </CardContent>
              <CardFooter className="border-t bg-muted/10">
                <Button
                  size="sm"
                  className="ml-auto w-full sm:w-auto"
                  onClick={() => handleResumeRoot(org.orgRootSessionId)}
                >
                  <Play className="mr-2 h-4 w-4" />
                  {t('orgHistory.resumeRoot', 'Resume Root Session')}
                </Button>
              </CardFooter>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
