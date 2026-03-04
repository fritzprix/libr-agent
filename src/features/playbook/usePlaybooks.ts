import { useState, useEffect, useCallback, useMemo } from 'react';
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
import { getLogger } from '@/lib/logger';
import type { PlaybookWithMeta, PlaybookSortState } from './types';

const logger = getLogger('usePlaybooks');

export function usePlaybooks(
  searchQuery: string,
  sortState: PlaybookSortState,
  onError?: (error: unknown) => void,
) {
  const [playbooks, setPlaybooks] = useState<PlaybookWithMeta[]>([]);
  const [assistants, setAssistants] = useState<
    Record<string, { name: string }>
  >({});
  const [loading, setLoading] = useState(true);
  const [isDeleting, setIsDeleting] = useState(false);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const [playbooksData, assistantsData] = await Promise.all([
        listPlaybooks({
          sortBy: sortState.sortMode,
          sortOrder: sortState.sortOrder,
          bookmarkFirst: sortState.bookmarkFirst,
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
      if (onError) onError(error);
      throw error; // Let caller handle toast if needed
    } finally {
      setLoading(false);
    }
  }, [
    sortState.sortMode,
    sortState.sortOrder,
    sortState.bookmarkFirst,
    onError,
  ]);

  useEffect(() => {
    fetchData().catch(() => {});
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
      // Revert on failure
      fetchData().catch(() => {});
      throw error;
    }
  };

  const confirmDelete = async (playbookToDelete: PlaybookWithMeta) => {
    if (!playbookToDelete) return;
    setIsDeleting(true);

    try {
      await deletePlaybook(playbookToDelete.id, playbookToDelete.agentId);
      setPlaybooks((prev) => prev.filter((p) => p.id !== playbookToDelete.id));
    } catch (error) {
      logger.error('Failed to delete playbook', error);
      throw error;
    } finally {
      setIsDeleting(false);
    }
  };

  // Filter and Process Playbooks
  const processedPlaybooks = useMemo(() => {
    const lowerQuery = searchQuery.toLowerCase();
    let filtered = playbooks.filter((p) => {
      return (
        p.goal.toLowerCase().includes(lowerQuery) ||
        (assistants[p.agentId]?.name || '').toLowerCase().includes(lowerQuery)
      );
    });
    return filtered;
  }, [playbooks, searchQuery, assistants]);

  const groups = useMemo(() => {
    if (sortState.groupMode === 'time') {
      return groupPlaybooksByTime(processedPlaybooks);
    } else if (sortState.groupMode === 'assistant') {
      return groupPlaybooksByAssistant(processedPlaybooks, assistants);
    }
    return null;
  }, [sortState.groupMode, processedPlaybooks, assistants]);

  const groupKeys = useMemo(() => {
    if (sortState.groupMode === 'none') return [];
    if (sortState.groupMode === 'time')
      return getGroupOrder('time').filter(
        (k) => groups?.[k] && groups[k].length > 0,
      );
    if (sortState.groupMode === 'assistant')
      return Object.keys(groups || {}).sort();
    return [];
  }, [sortState.groupMode, groups]);

  return {
    playbooks: processedPlaybooks,
    originalPlaybooksLength: playbooks.length,
    assistants,
    loading,
    isDeleting,
    groups,
    groupKeys,
    fetchData,
    handleBookmarkToggle,
    confirmDelete,
  };
}
