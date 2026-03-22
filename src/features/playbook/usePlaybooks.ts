import { useState, useMemo } from 'react';
import useSWR from 'swr';
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
  const [isDeleting, setIsDeleting] = useState(false);

  const {
    data: assistantsData,
    isLoading: isAssistantsLoading,
  } = useSWR(
    'assistants-map',
    async () => {
      const assistantsList = await listAssistants();
      return assistantsList.reduce<Record<string, { name: string }>>(
        (acc, curr) => {
          if (curr && curr.id) {
            acc[curr.id] = { name: curr.name };
          }
          return acc;
        },
        {},
      );
    },
    {
      onError: (error) => {
        logger.error('Failed to load assistants', error);
        if (onError) onError(error);
      },
    },
  );

  const {
    data: playbooksData,
    isLoading: isPlaybooksLoading,
    mutate: mutatePlaybooks,
  } = useSWR(
    [
      'playbooks',
      sortState.sortMode,
      sortState.sortOrder,
      sortState.bookmarkFirst,
    ],
    async ([, sortMode, sortOrder, bookmarkFirst]) => {
      return await listPlaybooks({
        sortBy: sortMode as "assistant" | "created_at" | undefined,
        sortOrder: sortOrder as "desc" | "asc" | undefined,
        bookmarkFirst: bookmarkFirst as boolean,
      });
    },
    {
      onError: (error) => {
        logger.error('Failed to load playbooks', error);
        if (onError) onError(error);
      },
    },
  );

  const assistants = assistantsData || {};
  const playbooks = playbooksData || [];
  const loading = isPlaybooksLoading || isAssistantsLoading;

  // Backwards compatibility for callers relying on fetchData
  const fetchData = async () => {
    await mutatePlaybooks();
  };

  const handleBookmarkToggle = async (
    id: string,
    isBookmarked: boolean,
    agentId: string,
  ) => {
    try {
      // Optimistic update
      await mutatePlaybooks(
        playbooks.map((p) => (p.id === id ? { ...p, isBookmarked } : p)),
        false,
      );

      await togglePlaybookBookmark(id, isBookmarked, agentId);
      await mutatePlaybooks();
    } catch (error) {
      logger.error('Failed to toggle bookmark', error);
      // Revert on failure
      await mutatePlaybooks();
      throw error;
    }
  };

  const confirmDelete = async (playbookToDelete: PlaybookWithMeta) => {
    if (!playbookToDelete) return;
    setIsDeleting(true);

    try {
      await deletePlaybook(playbookToDelete.id, playbookToDelete.agentId);
      await mutatePlaybooks(
        playbooks.filter((p) => p.id !== playbookToDelete.id),
        false,
      );
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
    return playbooks.filter((p) => {
      return (
        p.goal.toLowerCase().includes(lowerQuery) ||
        (assistants[p.agentId]?.name || '').toLowerCase().includes(lowerQuery)
      );
    });
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
