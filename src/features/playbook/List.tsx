import { useState, useMemo, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Search,
  Plus,
  RefreshCw,
  Play,
  PlayCircle as PlaybookIcon,
} from 'lucide-react';
import { getLogger } from '@/lib/logger';
import { usePlaybookService } from '@/lib/services/playbook-service';
import { Playbook } from '@/models/playbook';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { toast } from 'sonner';

const logger = getLogger('PlaybookList');

export default function PlaybookList() {
  const { t } = useTranslation();
  const { listPlaybooks, deletePlaybook, loading } = usePlaybookService();
  const [playbooks, setPlaybooks] = useState<Playbook[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [deleteConfirmId, setDeleteConfirmId] = useState<string | null>(null);

  const fetchPlaybooks = async () => {
    try {
      const data = await listPlaybooks();
      setPlaybooks(data);
    } catch (error) {
      logger.error('Failed to fetch playbooks', error);
      toast.error(t('playbook.toasts.loadFailed'));
    }
  };

  useEffect(() => {
    fetchPlaybooks();
  }, []);

  const onFetchDataWrapper = () => {
    fetchPlaybooks();
  };

  const filteredPlaybooks = useMemo(() => {
    if (!searchQuery.trim()) return playbooks;
    const query = searchQuery.toLowerCase();
    return playbooks.filter(
      (p) =>
        p.goal.toLowerCase().includes(query) ||
        p.assistant_id?.toLowerCase().includes(query),
    );
  }, [playbooks, searchQuery]);

  const handleDelete = async (id: string) => {
    try {
      await deletePlaybook(id);
      setPlaybooks((prev) => prev.filter((p) => p.id !== id));
      toast.success(t('playbook.toasts.deleted'));
    } catch (error) {
      logger.error('Failed to delete playbook', error);
      toast.error(t('playbook.toasts.deleteFailed'));
    } finally {
      setDeleteConfirmId(null);
    }
  };

  return (
    <div className="p-6 h-full flex flex-col bg-background">
      <div className="max-w-5xl mx-auto w-full flex flex-col h-full">
        {/* Header */}
        <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 mb-8">
          <div className="flex items-center gap-4">
            <div className="flex items-center justify-center p-2.5 bg-primary/10 text-primary rounded-xl">
              <PlaybookIcon size={28} />
            </div>
            <div>
              <h1 className="text-2xl text-foreground font-semibold tracking-tight">
                {t('playbook.title')}
              </h1>
              <p className="text-sm text-muted-foreground mt-0.5">
                {t('playbook.description')}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2 w-full sm:w-auto">
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="outline"
                  size="icon"
                  onClick={onFetchDataWrapper}
                  disabled={loading}
                  className="h-9 w-9"
                  aria-label={t(
                    'playbook.list.refreshAria',
                    'Refresh playbooks',
                  )}
                >
                  <RefreshCw
                    className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`}
                  />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {t('playbook.list.refreshTooltip', 'Refresh playbooks')}
              </TooltipContent>
            </Tooltip>
            <div className="relative flex-1 sm:w-64">
              <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
              <Input
                type="search"
                placeholder={t('playbook.searchPlaceholder')}
                className="pl-8 h-9"
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
        </div>

        {/* Content */}
        {loading && playbooks.length === 0 ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="flex flex-col items-center gap-3">
              <LoadingSpinner className="h-8 w-8 text-primary" />
              <p className="text-sm text-muted-foreground animate-pulse">
                {t('playbook.loading')}
              </p>
            </div>
          </div>
        ) : filteredPlaybooks.length === 0 ? (
          <div className="flex-1 flex items-center justify-center border-2 border-dashed border-muted rounded-3xl bg-muted/5">
            <div className="max-w-md text-center px-6">
              <div className="mx-auto w-12 h-12 bg-muted rounded-2xl flex items-center justify-center mb-4">
                <PlaybookIcon className="h-6 w-6 text-muted-foreground" />
              </div>
              <h3 className="text-lg font-medium text-foreground mb-2">
                {t('playbook.emptyState.title')}
              </h3>
              <p className="text-sm text-muted-foreground mb-6 leading-relaxed">
                {t('playbook.emptyState.description')}
              </p>
              <div className="space-y-3 text-left bg-muted/50 p-4 rounded-2xl border border-muted/50">
                <p className="text-xs text-muted-foreground flex gap-2">
                  <span className="flex-shrink-0 w-4 h-4 rounded-full bg-primary/10 text-primary flex items-center justify-center font-bold">
                    1
                  </span>
                  {t('playbook.emptyState.step1')}
                </p>
                <p className="text-xs text-muted-foreground flex gap-2">
                  <span className="flex-shrink-0 w-4 h-4 rounded-full bg-primary/10 text-primary flex items-center justify-center font-bold">
                    2
                  </span>
                  {t('playbook.emptyState.step2')}
                </p>
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 overflow-auto pr-2 -mr-2">
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6 pb-6">
              {filteredPlaybooks.map((playbook) => (
                <Card
                  key={playbook.id}
                  className="group relative border-muted/60 hover:border-primary/30 transition-all duration-300 bg-card/50 hover:bg-card hover:shadow-lg hover:shadow-primary/5 rounded-3xl overflow-hidden"
                >
                  <CardHeader className="pb-3">
                    <div className="flex justify-between items-start mb-2">
                      <div className="p-2 bg-primary/10 text-primary rounded-xl group-hover:bg-primary group-hover:text-white transition-colors duration-300">
                        <Play size={18} />
                      </div>
                      <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                        <Button
                          variant="ghost"
                          size="icon"
                          className="h-8 w-8 text-muted-foreground hover:text-destructive rounded-lg"
                          onClick={() => setDeleteConfirmId(playbook.id)}
                        >
                          <Plus className="h-4 w-4 rotate-45" />
                        </Button>
                      </div>
                    </div>
                    <CardTitle className="text-lg font-semibold line-clamp-2 leading-tight">
                      {playbook.goal}
                    </CardTitle>
                  </CardHeader>
                  <CardContent>
                    <div className="flex flex-col gap-4">
                      <div className="flex flex-wrap gap-2">
                        <div className="text-[10px] font-medium uppercase tracking-wider text-muted-foreground bg-muted/50 px-2.5 py-1 rounded-full border border-muted/50">
                          ID: {playbook.id.substring(0, 8)}
                        </div>
                        <div className="text-[10px] font-medium uppercase tracking-wider text-primary bg-primary/10 px-2.5 py-1 rounded-full border border-primary/10">
                          {playbook.steps.length} Steps
                        </div>
                      </div>

                      <div className="flex items-center justify-between pt-4 border-t border-muted/40">
                        <span className="text-xs text-muted-foreground">
                          {playbook.assistant_id || 'Global'}
                        </span>
                        <Button
                          size="sm"
                          className="rounded-xl px-4 h-8 text-xs font-medium"
                        >
                          {t('playbook.card.start')}
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </div>
        )}
      </div>

      <AlertDialog
        open={!!deleteConfirmId}
        onOpenChange={() => setDeleteConfirmId(null)}
      >
        <AlertDialogContent className="rounded-3xl border-muted/60">
          <AlertDialogHeader>
            <AlertDialogTitle>{t('playbook.deleteDialog.title')}</AlertDialogTitle>
            <AlertDialogDescription>
              {t('playbook.deleteDialog.description', {
                goal: playbooks.find((p) => p.id === deleteConfirmId)?.goal,
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter className="gap-2">
            <AlertDialogCancel className="rounded-xl">
              {t('playbook.deleteDialog.cancel')}
            </AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive hover:bg-destructive/90 rounded-xl"
              onClick={() => deleteConfirmId && handleDelete(deleteConfirmId)}
            >
              {t('playbook.deleteDialog.confirm')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
