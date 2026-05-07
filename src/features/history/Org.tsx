import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Building2, RefreshCw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { cn } from '@/lib/utils';
import { safeInvoke } from '@/lib/backend/core';
import type { AgentSession } from '@/models/agent';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import { mapSessionMetadataToAgentSession } from '@/lib/session-metadata';
import { selectOrgSummaries } from './org-sessions';
import { OrgCard } from './OrgCard';

function OrgCardSkeleton() {
  return (
    <Card className="overflow-hidden">
      <CardHeader className="border-b bg-muted/20">
        <Skeleton className="h-6 w-32" />
        <Skeleton className="h-4 w-40" />
      </CardHeader>
      <CardContent className="space-y-4 pt-6">
        <div className="grid gap-3 sm:grid-cols-3">
          <Skeleton className="h-20 rounded-xl" />
          <Skeleton className="h-20 rounded-xl" />
          <Skeleton className="h-20 rounded-xl" />
        </div>
        <Skeleton className="h-16 w-full" />
      </CardContent>
    </Card>
  );
}

export default function Org() {
  const { t } = useTranslation('common');
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [isSessionsListLoading, setIsSessionsListLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const orgs = useMemo(() => selectOrgSummaries(sessions), [sessions]);

  async function loadOrgSessions(forceRefreshing = false) {
    if (forceRefreshing) {
      setIsRefreshing(true);
    } else {
      setIsSessionsListLoading(true);
    }
    try {
      const response = await safeInvoke<AgentSessionMetadata[]>(
        'agent_get_all_sessions',
      );
      const items = Array.isArray(response) ? response : [];
      setSessions(
        items.map((session) => mapSessionMetadataToAgentSession(session)),
      );
    } finally {
      setIsRefreshing(false);
      setIsSessionsListLoading(false);
    }
  }

  useEffect(() => {
    void loadOrgSessions();
  }, []);

  async function handleRefresh() {
    await loadOrgSessions(true);
  }

  if (isSessionsListLoading) {
    return (
      <div className="mx-auto flex max-w-6xl flex-col gap-6 p-6">
        <div className="flex items-center justify-between">
          <div>
            <Skeleton className="mb-2 h-8 w-40" />
            <Skeleton className="h-4 w-96" />
          </div>
          <Skeleton className="h-9 w-24" />
        </div>
        <div className="grid gap-4 lg:grid-cols-2">
          {Array.from({ length: 3 }).map((_, i) => (
            <OrgCardSkeleton key={i} />
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
          <p className="mt-1 text-sm text-muted-foreground">
            {t(
              'orgHistory.description',
              'Explicit org-created lineages only. One-off delegated sessions stay in ordinary history.',
            )}
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={handleRefresh}
          disabled={isRefreshing}
        >
          <RefreshCw
            className={cn('mr-2 h-4 w-4', isRefreshing && 'animate-spin')}
          />
          {t('history.refresh', 'Refresh')}
        </Button>
      </div>

      {orgs.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border/70 bg-muted/10 py-20 text-center text-muted-foreground">
          <Building2 className="h-10 w-10 opacity-50" />
          <p className="text-base font-medium">
            {t('orgHistory.emptyState.title', 'No org lineages yet')}
          </p>
          <p className="max-w-xl text-sm">
            {t(
              'orgHistory.emptyState.subtitle',
              'Create an org explicitly, then spawn org members through the org-aware tools. Plain sub-agent lineage does not belong here.',
            )}
          </p>
        </div>
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          {orgs.map((org) => (
            <OrgCard key={org.orgId} org={org} />
          ))}
        </div>
      )}
    </div>
  );
}
