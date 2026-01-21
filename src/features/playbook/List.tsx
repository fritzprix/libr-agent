import { useState, useEffect, useMemo, useCallback } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
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
import { Search, RefreshCw, Loader2 } from 'lucide-react';
import { getLogger } from '@/lib/logger';
import { Playbook } from '@/types/playbook';

const logger = getLogger('PlaybookList');

// Type for playbooks with metadata
type PlaybookWithMeta = Playbook & {
  id: string;
  createdAt: Date;
  sessionId: string;
  updatedAt: Date;
};

export default function PlaybookList() {
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
      toast.error('Failed to load playbooks');
    } finally {
      setLoading(false);
    }
  }, [sortMode, sortOrder, bookmarkFirst]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleBookmarkToggle = async (
    id: string,
    isBookmarked: boolean,
    sessionId: string,
  ) => {
    try {
      // Optimistic update
      setPlaybooks((prev) =>
        prev.map((p) => (p.id === id ? { ...p, isBookmarked } : p)),
      );

      await togglePlaybookBookmark(id, isBookmarked, sessionId);
    } catch (error) {
      logger.error('Failed to toggle bookmark', error);
      toast.error('Failed to update bookmark');
      fetchData(); // Revert on failure
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Are you sure you want to delete this playbook?')) return;
    try {
      await deletePlaybook(id);
      setPlaybooks((prev) => prev.filter((p) => p.id !== id));
      toast.success('Playbook deleted');
    } catch (error) {
      logger.error('Failed to delete playbook', error);
      toast.error('Failed to delete playbook');
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

  return (
    <div className="container mx-auto p-6 h-full flex flex-col min-h-0 bg-background">
      <div className="flex flex-col sm:flex-row justify-between items-start sm:items-center gap-4 mb-6">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Playbooks</h1>
          <p className="text-muted-foreground mt-1">
            Browse and execute automated workflows
          </p>
        </div>

        <div className="flex items-center gap-2 w-full sm:w-auto">
          <Button
            variant="outline"
            size="icon"
            onClick={() => fetchData()}
            disabled={loading}
          >
            <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin' : ''}`} />
          </Button>
          <div className="relative flex-1 sm:w-[250px]">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              type="search"
              placeholder="Search playbooks..."
              className="pl-8"
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

      <div className="flex-1 overflow-y-auto min-h-0 pr-1">
        {loading && playbooks.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-[50vh] text-muted-foreground">
            <Loader2 className="h-10 w-10 animate-spin mb-4" />
            <p>Loading playbooks...</p>
          </div>
        ) : processedPlaybooks.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-[50vh] text-muted-foreground">
            <p>No playbooks found matching your criteria.</p>
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
                      assistants[playbook.agentId]?.name || 'Unknown'
                    }
                    onBookmarkToggle={(id, val) =>
                      handleBookmarkToggle(id, val, playbook.sessionId)
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
                      title={key}
                      playbooks={groups[key]}
                      assistantMap={assistants}
                      onBookmarkToggle={(id, val) =>
                        handleBookmarkToggle(
                          id,
                          val,
                          groups[key].find((p) => p.id === id)?.sessionId || '',
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
    </div>
  );
}
