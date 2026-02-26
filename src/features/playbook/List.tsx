import { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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
import { SortControls, SortMode, SortOrder, GroupMode } from './SortControls';
import {
  listPlaybooks,
  deletePlaybook,
  togglePlaybookBookmark,
} from '@/lib/backend/playbooks';
import { listAssistants } from '@/lib/backend/assistants';
import {
  groupPlaybooksByTime,
  groupPlaybooksByAssistant,
  getGroupOrder,
} from './grouping-utils';
import { toast } from 'sonner';
import { Search, RefreshCw, Loader2, Book as PlaybookIcon } from 'lucide-react';
import { getLogger } from '@/lib/logger';
import { Playbook } from '@/types/playbook';

const logger = getLogger('PlaybookList');

// Type for playbooks with metadata
type PlaybookWithMeta = Playbook & {
  id: string;
  createdAt: Date;
  updatedAt: Date;
};

export default function PlaybookList() {
  const { t } = useTranslation();
  const [playbooks, setPlaybooks] = useState<PlaybookWithMeta[]>([]);
  const [assistants, setAssistants] = useState<
    Record<string, { name: string }>
  >({});
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');

  const [sortMode, setSortMode] = useState<SortMode>('created_at');
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc');
  const [groupMode, setGroupMode] = useState<GroupMode>('none');
  const [bookmarkFirst, setBookmarkFirst] = useState(false);
  const [playbookToDelete, setPlaybookToDelete] =
    useState<PlaybookWithMeta | null>(null);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const [playbooksData, assistantsData] = await Promise.all([
        listPlaybooks({
          sortBy: sortMode,
          sortOrder: sortOrder,
          bookmarkFirst: bookmarkFirst,
        }),
        listAssistants(),
      ]);

      setPlaybooks(playbooksData);

      const assistantMap = assistantsData.reduce<
        Record<string, { name: string }>
      >((acc, curr) => {
        if (curr && curr.id) {
          acc[curr.id] = { name: curr.name };
        }
        return acc;
      }, {});
      setAssistants(assistantMap);
    } catch (error) {
      logger.error('Failed to load playbooks', error);
      toast.error(t('playbook.toasts.loadFailed'));
    } finally {
      setLoading(false);
    }
  }, [sortMode, sortOrder, bookmarkFirst, t]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleBookmarkToggle = async (
    id: string,
    isBookmarked: boolean,
    agentId: string,
  ) => {
    try {
      // Optimistic update
      setPlaybooks((prev) =>
        prev.map((p) => (p.id === id ? { ...p, isBookmarked } : p)),
      );

      await togglePlaybookBookmark(id, isBookmarked, agentId);
    } catch (error) {
      logger.error('Failed to toggle bookmark', error);
      toast.error(t('playbook.toasts.bookmarkFailed'));
      fetchData(); // Revert on failure
    }
  };

  const handleDelete = (id: string) => {
    const playbook = playbooks.find((p) => p.id === id);
    if (playbook) {
      setPlaybookToDelete(playbook);
    }
  };

  const confirmDelete = async () => {
    if (!playbookToDelete) return;

    try {
      await deletePlaybook(playbookToDelete.id, playbookToDelete.agentId);
      setPlaybooks((prev) => prev.filter((p) => p.id !== playbookToDelete.id));
      toast.success(t('playbook.toasts.deleted'));
    } catch (error) {
      logger.error('Failed to delete playbook', error);
      toast.error(t('playbook.toasts.deleteFailed'));
    } finally {
      setPlaybookToDelete(null);
    }
  };

  // Filter and Process Playbooks
  const processedPlaybooks = useMemo(() => {
    let filtered = playbooks.filter((p) => {
      const query = searchQuery.toLowerCase();
      return (
        p.goal.toLowerCase().includes(query) ||
        (assistants[p.agentId]?.name || '').toLowerCase().includes(query)
      );
    });
    return filtered;
  }, [playbooks, searchQuery, assistants]);

  const groups = useMemo(() => {
    if (groupMode === 'time') {
      return groupPlaybooksByTime(processedPlaybooks);
    } else if (groupMode === 'assistant') {
      return groupPlaybooksByAssistant(processedPlaybooks, assistants);
    }
    return null;
  }, [groupMode, processedPlaybooks, assistants]);

  const groupKeys = useMemo(() => {
    if (groupMode === 'none') return [];
    if (groupMode === 'time')
      return getGroupOrder('time').filter(
        (k) => groups?.[k] && groups[k].length > 0,
      );
    if (groupMode === 'assistant') return Object.keys(groups || {}).sort();
    return [];
  }, [groupMode, groups]);

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
              onClick={() => fetchData()}
              disabled={loading}
              className="h-9 w-9"
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
            <SortControls
              sortMode={sortMode}
              setSortMode={setSortMode}
              sortOrder={sortOrder}
              setSortOrder={setSortOrder}
              groupMode={groupMode}
              setGroupMode={setGroupMode}
              bookmarkFirst={bookmarkFirst}
              onBookmarkFirstToggle={() => setBookmarkFirst(!bookmarkFirst)}
            />
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto pr-2 pb-4">
          {loading && playbooks.length === 0 ? (
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
              {groupMode === 'none' ? (
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
                        handleBookmarkToggle(id, val, playbook.agentId)
                      }
                      onDelete={handleDelete}
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
                          groupMode === 'time' || key.startsWith('playbook.')
                            ? t(key)
                            : key
                        }
                        playbooks={groups[key]}
                        assistantMap={assistants}
                        onBookmarkToggle={(id, val) =>
                          handleBookmarkToggle(
                            id,
                            val,
                            groups[key].find((p) => p.id === id)?.agentId || '',
                          )
                        }
                        onDelete={handleDelete}
                      />
                    ),
                )
              )}
            </div>
          )}
        </div>

        <AlertDialog
          open={!!playbookToDelete}
          onOpenChange={(open) => !open && setPlaybookToDelete(null)}
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
              <AlertDialogCancel>
                {t('playbook.deleteDialog.cancel')}
              </AlertDialogCancel>
              <AlertDialogAction onClick={confirmDelete}>
                {t('playbook.deleteDialog.confirm')}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </div>
    </div>
  );
}
