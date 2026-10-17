import { useDeferredValue, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import useSWR from 'swr';
import { Building2, RefreshCw, Search, Upload, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import { cn } from '@/lib/utils';
import { safeInvoke } from '@/lib/backend/core';
import type { AgentSessionMetadata } from '@/models/agent-ipc';
import { mapSessionMetadataToAgentSession } from '@/lib/session-metadata';
import { selectOrgSummaries } from './org-sessions';
import { OrgCard } from './OrgCard';
import { useOrgBundle } from './useOrgBundle';

const logger = getLogger('OrgHistory');

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
  const [searchQuery, setSearchQuery] = useState('');
  const deferredSearchQuery = useDeferredValue(searchQuery);
  const { busy: orgBundleBusy, exportOrg, importOrg } = useOrgBundle();

  const fetcher = async () => {
    const response = await safeInvoke<AgentSessionMetadata[]>(
      'agent_get_all_sessions',
    );
    const items = Array.isArray(response) ? response : [];
    return items.map((session) => mapSessionMetadataToAgentSession(session));
  };

  const {
    data: sessions = [],
    isLoading,
    isValidating,
    mutate,
  } = useSWR('orgSessions', fetcher, {
    revalidateOnFocus: false,
    onError: (error) => {
      logger.error('Failed to load org sessions', error);
      toast.error(t('orgHistory.loadFailed', 'Failed to load org lineages'));
    },
  });

  const orgs = useMemo(() => selectOrgSummaries(sessions), [sessions]);

  const filteredOrgs = useMemo(() => {
    const query = deferredSearchQuery.trim().toLowerCase();
    if (!query) {
      return orgs;
    }

    return orgs.filter(
      (org) =>
        org.orgName.toLowerCase().includes(query) ||
        org.orgId.toLowerCase().includes(query) ||
        org.orgRootSessionId.toLowerCase().includes(query),
    );
  }, [deferredSearchQuery, orgs]);

  async function handleRefresh() {
    await mutate();
  }

  async function handleExportOrg(orgId: string, orgName: string) {
    try {
      const result = await exportOrg(orgId, orgName);
      if (!result) {
        return;
      }
      toast.success(
        t('orgHistory.toasts.exported', 'Organization bundle exported'),
      );
    } catch (error) {
      logger.error('Org export failed', error);
      toast.error(
        t('orgHistory.toasts.exportFailed', 'Failed to export organization'),
      );
    }
  }

  async function handleImportOrg() {
    try {
      const result = await importOrg();
      if (!result) {
        return;
      }
      const skillsHint = result.workspace_skills_import_path
        ? ` | workspace skills: ${result.workspace_skills_import_path}`
        : '';
      toast.success(
        t(
          'orgHistory.toasts.imported',
          'Organization bundle imported. Teamwork files: {{path}}',
          { path: `${result.teamwork_import_path}${skillsHint}` },
        ),
      );
    } catch (error) {
      logger.error('Org import failed', error);
      const message = error instanceof Error ? error.message : String(error);
      toast.error(
        message.includes('PASSWORD_REQUIRED')
          ? t(
              'orgHistory.toasts.importPasswordRequired',
              'This Org bundle is encrypted. Password-protected import is not available in this UI yet — use an unencrypted export or Migration tooling.',
            )
          : t(
              'orgHistory.toasts.importFailed',
              'Failed to import organization bundle',
            ),
      );
    }
  }

  if (isLoading) {
    return (
      <div className="flex min-h-full w-full flex-col bg-background p-6 text-foreground">
        <div className="mx-auto w-full max-w-6xl">
          <div className="mb-6 flex items-center justify-between">
            <div>
              <Skeleton className="mb-2 h-8 w-40" />
              <Skeleton className="h-4 w-96" />
            </div>
            <Skeleton className="h-9 w-24" />
          </div>
          <div className="grid gap-4 pb-4 lg:grid-cols-2">
            {Array.from({ length: 3 }).map((_, i) => (
              <OrgCardSkeleton key={i} />
            ))}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-full w-full flex-col bg-background p-6 text-foreground">
      <div className="mx-auto w-full max-w-6xl">
        <div className="mb-6 flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
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
          <div className="flex shrink-0 flex-wrap gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleImportOrg}
              disabled={orgBundleBusy || isValidating}
            >
              <Upload className="mr-2 h-4 w-4" />
              {t('orgHistory.importBundle', 'Import Org')}
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={handleRefresh}
              disabled={isValidating}
            >
              <RefreshCw
                className={cn('mr-2 h-4 w-4', isValidating && 'animate-spin')}
              />
              {t('history.refresh', 'Refresh')}
            </Button>
          </div>
        </div>

        {orgs.length > 0 ? (
          <div className="relative mb-4 max-w-md">
            <Search className="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder={t(
                'orgHistory.searchPlaceholder',
                'Search orgs by name or ID…',
              )}
              aria-label={t('orgHistory.searchAria', 'Search organizations')}
              className="pr-9 pl-9"
            />
            {searchQuery ? (
              <button
                type="button"
                className="absolute top-1/2 right-2 inline-flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-md text-muted-foreground hover:text-foreground"
                aria-label={t('orgHistory.clearSearchAria', 'Clear search')}
                onClick={() => setSearchQuery('')}
              >
                <X className="h-4 w-4" />
              </button>
            ) : null}
          </div>
        ) : null}

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
        ) : filteredOrgs.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 rounded-2xl border border-dashed border-border/70 bg-muted/10 py-16 text-center text-muted-foreground">
            <p className="text-base font-medium">
              {t(
                'orgHistory.noOrgsMatching',
                'No organizations match "{{query}}"',
                { query: searchQuery.trim() },
              )}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => setSearchQuery('')}
            >
              {t('orgHistory.clearSearch', 'Clear search')}
            </Button>
          </div>
        ) : (
          <div className="grid gap-4 pb-4 lg:grid-cols-2">
            {filteredOrgs.map((org) => (
              <OrgCard
                key={org.orgId}
                org={org}
                exportBusy={orgBundleBusy}
                onExport={handleExportOrg}
                onDeleted={async () => {
                  await mutate();
                }}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
