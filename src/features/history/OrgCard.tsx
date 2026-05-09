import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Building2, Play, Trash2, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { formatSessionTimestamp } from '@/lib/date-utils';
import { cn } from '@/lib/utils';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { toast } from 'sonner';
import { getLogger } from '@/lib/logger';
import type { OrgSummary } from './org-sessions';
import { getStatusBadgeConfig } from './org-status';
import { OrgStatTiles } from './OrgStatTiles';
import { OrgLineageSnapshot } from './OrgLineageSnapshot';

const logger = getLogger('OrgCard');

interface OrgCardProps {
  org: OrgSummary;
  onDeleted: () => Promise<void>;
}

export function OrgCard({ org, onDeleted }: OrgCardProps) {
  const navigate = useNavigate();
  const { t } = useTranslation('common');
  const { deleteSession } = useAgentSessionListActions();
  const [isDeleting, setIsDeleting] = useState(false);
  const [isDialogOpen, setIsDialogOpen] = useState(false);

  const ts = formatSessionTimestamp(org.updatedAt);
  const rootBadge = getStatusBadgeConfig(org.rootSession.status);

  async function handleDelete(e: React.MouseEvent) {
    e.preventDefault();

    setIsDeleting(true);
    try {
      await deleteSession(org.orgRootSessionId);
      toast.success(t('orgHistory.toasts.deleted', 'Organization deleted'));

      try {
        await onDeleted();
      } catch (refreshError) {
        logger.warn(
          'Organization deleted but failed to refresh org history',
          refreshError,
        );
      }

      setIsDialogOpen(false);
    } catch (error) {
      logger.error('Failed to delete organization', error);
      toast.error(
        t('orgHistory.toasts.deleteFailed', 'Failed to delete organization'),
      );
    } finally {
      setIsDeleting(false);
    }
  }

  return (
    <Card className="overflow-hidden border-border/70 bg-card shadow-sm shadow-black/5 transition-shadow hover:shadow-md">
      <CardHeader className="flex-row items-start justify-between gap-4 overflow-hidden border-b bg-muted/20 pb-4">
        <div className="min-w-0 flex-1 space-y-3">
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

        <AlertDialog open={isDialogOpen} onOpenChange={setIsDialogOpen}>
          <AlertDialogTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="mt-1 h-8 w-8 shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              disabled={isDeleting}
            >
              <Trash2 className="h-4 w-4" />
              <span className="sr-only">
                {t('orgHistory.delete', 'Delete Organization')}
              </span>
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>
                {t('orgHistory.deleteConfirm.title', 'Delete Organization?')}
              </AlertDialogTitle>
              <AlertDialogDescription>
                {t(
                  'orgHistory.deleteConfirm.description',
                  'This will permanently delete this organization and all of its associated member sessions, including all files and histories. This action cannot be undone.',
                )}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={isDeleting}>
                {t('common.cancel')}
              </AlertDialogCancel>
              <AlertDialogAction
                onClick={handleDelete}
                disabled={isDeleting}
                className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
              >
                {isDeleting ? (
                  <>
                    <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                    {t('common.delete', 'Delete')}
                  </>
                ) : (
                  t('common.delete', 'Delete')
                )}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
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
