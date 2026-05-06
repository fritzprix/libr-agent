import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  AlertTriangle,
  FileText,
  Loader2,
  Network,
  Trash2,
} from 'lucide-react';
import {
  Button,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from '@/components/ui';
import { Skeleton } from '@/components/ui/skeleton';
import type {
  KnowledgeChunkDetail,
  KnowledgeChunkListItem,
} from '@/lib/backend/knowledge';
import { KnowledgeDetailGraphTab } from './knowledge-detail/KnowledgeDetailGraphTab';
import { KnowledgeDetailOverviewTab } from './knowledge-detail/KnowledgeDetailOverviewTab';

interface KnowledgeDetailDialogProps {
  open: boolean;
  detail: KnowledgeChunkDetail | null;
  isDeleteConfirming: boolean;
  isDeleting: boolean;
  isDetailLoading: boolean;
  onCancelDelete: () => void;
  onClose: () => void;
  onRequestDelete: () => void;
  selectedItem: KnowledgeChunkListItem | null;
}

export const KnowledgeDetailDialog = memo(function KnowledgeDetailDialog({
  open,
  detail,
  isDeleteConfirming,
  isDeleting,
  isDetailLoading,
  onCancelDelete,
  onClose,
  onRequestDelete,
  selectedItem,
}: KnowledgeDetailDialogProps) {
  const { t } = useTranslation('common');

  if (!open) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={(isOpen) => !isOpen && onClose()}>
      <DialogContent
        showCloseButton={false}
        className="grid h-[92vh] w-[calc(100vw-1.5rem)] max-w-[calc(100vw-1.5rem)] grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden border border-border/50 bg-background p-0 shadow-[0_28px_80px_-36px_rgba(0,0,0,0.45)] sm:!max-w-[min(1500px,calc(100vw-1.5rem))]"
      >
        <DialogHeader className="border-b border-border/40 px-6 py-5 text-left">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div>
              <DialogTitle className="text-base">
                {t('knowledge.detailTitle', 'Knowledge Detail')}
              </DialogTitle>
              <DialogDescription>
                {t(
                  'knowledge.detailDescription',
                  'Inspect the selected entry, its evidence, and its local graph.',
                )}
              </DialogDescription>
            </div>

            <div className="flex flex-wrap gap-2">
              {isDeleteConfirming ? (
                <>
                  <Button
                    type="button"
                    variant="outline"
                    onClick={onCancelDelete}
                    disabled={isDeleting}
                  >
                    {t('knowledge.cancel', 'Cancel')}
                  </Button>
                  <Button
                    type="button"
                    variant="destructive"
                    onClick={onRequestDelete}
                    disabled={!selectedItem || isDeleting}
                    className="gap-2"
                  >
                    {isDeleting ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <Trash2 className="h-4 w-4" />
                    )}
                    {t('knowledge.confirmDeleteAction', 'Delete permanently')}
                  </Button>
                </>
              ) : (
                <>
                  <Button type="button" variant="outline" onClick={onClose}>
                    {t('knowledge.close', 'Close')}
                  </Button>
                  <Button
                    type="button"
                    variant="destructive"
                    onClick={onRequestDelete}
                    disabled={!selectedItem || isDeleting}
                    className="gap-2"
                  >
                    <Trash2 className="h-4 w-4" />
                    {t('knowledge.delete', 'Delete')}
                  </Button>
                </>
              )}
            </div>
          </div>
          {isDeleteConfirming ? (
            <div className="mt-4 flex items-start gap-3 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-foreground">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
              <div className="space-y-1">
                <p className="font-medium">
                  {t('knowledge.confirmDeleteTitle', 'Delete knowledge entry')}
                </p>
                <p className="text-muted-foreground">
                  {t(
                    'knowledge.confirmDelete',
                    'Delete this knowledge entry and clean up orphaned graph data?',
                  )}
                </p>
              </div>
            </div>
          ) : null}
        </DialogHeader>

        <div className="min-h-0 overflow-hidden px-6 py-5">
          {isDetailLoading || !detail ? (
            <div className="space-y-4">
              <Skeleton className="h-6 w-40" />
              <Skeleton className="h-28 w-full" />
              <Skeleton className="h-48 w-full" />
            </div>
          ) : (
            <Tabs
              defaultValue="overview"
              className="flex h-full flex-col gap-4"
            >
              <TabsList className="grid w-full max-w-md grid-cols-2">
                <TabsTrigger value="overview" className="gap-2">
                  <FileText className="h-4 w-4" />
                  {t('knowledge.tabs.overview', 'Overview')}
                </TabsTrigger>
                <TabsTrigger value="graph" className="gap-2">
                  <Network className="h-4 w-4" />
                  {t('knowledge.tabs.graph', 'Graph')}
                </TabsTrigger>
              </TabsList>

              <TabsContent value="overview" className="min-h-0 flex-1">
                <KnowledgeDetailOverviewTab detail={detail} />
              </TabsContent>

              <TabsContent value="graph" className="min-h-0 flex-1">
                <KnowledgeDetailGraphTab detail={detail} />
              </TabsContent>
            </Tabs>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
});
