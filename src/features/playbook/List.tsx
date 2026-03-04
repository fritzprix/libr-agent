import {
  useState,
  useCallback,
  useDeferredValue,
  type MouseEvent,
} from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
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
import { PlaybookCard } from './Card';
import { PlaybookGroup } from './PlaybookGroup';
import { SortControls } from './SortControls';
import { toast } from 'sonner';
import { Search, RefreshCw, Loader2, Book as PlaybookIcon } from 'lucide-react';
import type { PlaybookWithMeta } from './grouping-utils';
import { usePlaybooks } from './usePlaybooks';
import type { PlaybookSortState } from './types';

export default function PlaybookList() {
  const { t } = useTranslation();
  const [searchQuery, setSearchQuery] = useState('');
  const deferredSearchQuery = useDeferredValue(searchQuery);

  const [sortState, setSortState] = useState<PlaybookSortState>({
    sortMode: 'created_at',
    sortOrder: 'desc',
    groupMode: 'none',
    bookmarkFirst: false,
  });

  const handleError = useCallback(() => {
    toast.error(t('playbook.toasts.loadFailed'));
  }, [t]);

  const {
    playbooks: processedPlaybooks,
    originalPlaybooksLength,
    assistants,
    loading,
    isDeleting,
    groups,
    groupKeys,
    fetchData,
    handleBookmarkToggle,
    confirmDelete: _confirmDelete,
  } = usePlaybooks(deferredSearchQuery, sortState, handleError);

  const [playbookToDelete, setPlaybookToDelete] =
    useState<PlaybookWithMeta | null>(null);

  const handleDeleteClick = (id: string) => {
    // We could find the playbook from original list or processed list.
    const playbook = processedPlaybooks.find((p) => p.id === id);
    if (playbook) {
      setPlaybookToDelete(playbook);
    }
  };

  const handleConfirmDelete = async (e: MouseEvent) => {
    e.preventDefault();
    if (!playbookToDelete) return;

    try {
      await _confirmDelete(playbookToDelete);
      toast.success(t('playbook.toasts.deleted'));
      setPlaybookToDelete(null);
    } catch {
      toast.error(t('playbook.toasts.deleteFailed'));
    }
  };

  const onBookmarkToggleWrapper = async (
    id: string,
    val: boolean,
    agentId: string,
  ) => {
    try {
      await handleBookmarkToggle(id, val, agentId);
    } catch {
      toast.error(t('playbook.toasts.bookmarkFailed'));
    }
  };

  const onFetchDataWrapper = async () => {
    try {
      await fetchData();
    } catch {
      toast.error(t('playbook.toasts.loadFailed'));
    }
  };

  // Render list regardless of session state

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
            <Button
              variant="outline"
              size="icon"
              onClick={onFetchDataWrapper}
              disabled={loading}
              className="h-9 w-9"
              aria-label={t('playbook.list.refreshAria', 'Refresh playbooks')}
            >
              <RefreshCw
                className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`}
              />
            </Button>
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
            <SortControls sortState={sortState} setSortState={setSortState} />
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          {loading && originalPlaybooksLength === 0 ? (
            <div className="flex flex-col items-center justify-center h-[50vh] text-muted-foreground">
              <Loader2 className="h-10 w-10 animate-spin mb-4" />
              <p>{t('playbook.loading')}</p>
            </div>
          ) : processedPlaybooks.length === 0 ? (
            <div className="flex flex-col items-center justify-center p-8 text-center max-w-2xl mx-auto">
              <Card className="w-full bg-card/50 backdrop-blur-sm border-dashed">
                <CardHeader>
                  <div className="mx-auto bg-primary/10 p-4 rounded-full mb-4 w-16 h-16 flex items-center justify-center">
                    <PlaybookIcon className="w-8 h-8 text-primary" />
                  </div>
                  <CardTitle className="text-2xl font-bold">
                    {t('playbook.emptyState.title')}
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-6 text-muted-foreground">
                  <p>{t('playbook.emptyState.description')}</p>
                  <div className="grid gap-4 text-left p-4 bg-muted/50 rounded-lg">
                    <div className="flex gap-3">
                      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">
                        1
                      </span>
                      <p className="text-sm">
                        {t('playbook.emptyState.step1')}
                      </p>
                    </div>
                    <div className="flex gap-3">
                      <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-primary text-xs font-bold text-primary-foreground">
                        2
                      </span>
                      <p className="text-sm">
                        {t('playbook.emptyState.step2')}
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
            </div>
          ) : (
            <div className="space-y-8 pb-8">
              {sortState.groupMode === 'none' ? (
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                  {processedPlaybooks.map((playbook) => (
                    <PlaybookCard
                      key={playbook.id}
                      playbook={playbook}
                      assistantName={
                        assistants[playbook.agentId]?.name ||
                        t('playbook.card.unknownAssistant')
                      }
                      onBookmarkToggle={(id, val) =>
                        onBookmarkToggleWrapper(id, val, playbook.agentId)
                      }
                      onDelete={handleDeleteClick}
                    />
                  ))}
                </div>
              ) : (
                groupKeys.map(
                  (key) =>
                    groups &&
                    groups[key] && (
                      <PlaybookGroup
                        key={key}
                        title={
                          sortState.groupMode === 'time' || key.startsWith('playbook.')
                            ? t(key)
                            : key
                        }
                        playbooks={groups[key]}
                        assistantMap={assistants}
                        onBookmarkToggle={(id, val) =>
                          onBookmarkToggleWrapper(
                            id,
                            val,
                            groups[key].find((p) => p.id === id)?.agentId || '',
                          )
                        }
                        onDelete={handleDeleteClick}
                      />
                    ),
                )
              )}
            </div>
          )}
        </div>

        <AlertDialog
          open={!!playbookToDelete}
          onOpenChange={(open) =>
            !open && !isDeleting && setPlaybookToDelete(null)
          }
        >
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>
                {t('playbook.deleteDialog.title')}
              </AlertDialogTitle>
              <AlertDialogDescription>
                {t('playbook.deleteDialog.description', {
                  goal: playbookToDelete?.goal,
                })}
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={isDeleting}>
                {t('playbook.deleteDialog.cancel')}
              </AlertDialogCancel>
              <AlertDialogAction onClick={handleConfirmDelete} disabled={isDeleting}>
                {isDeleting && <LoadingSpinner className="mr-2 h-4 w-4" />}
                {t('playbook.deleteDialog.confirm')}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </div>
  );
}
