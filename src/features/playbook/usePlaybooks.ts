import { useState, useCallback, useMemo } from 'react';
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

  const fetchPlaybooksAndAssistants = async () => {
    const [playbooksData, assistantsData] = await Promise.all([
      listPlaybooks({
        sortBy: sortState.sortMode,
        sortOrder: sortState.sortOrder,
        bookmarkFirst: sortState.bookmarkFirst,
      }),
      listAssistants(),
    ]);

    const assistantMap = assistantsData.reduce<
      Record<string, { name: string }>
    >((acc, curr) => {
      if (curr && curr.id) {
        acc[curr.id] = { name: curr.name };
      }
      return acc;
    }, {});

    return { playbooks: playbooksData, assistants: assistantMap };
  };

  const swrKey = [
    'playbooks',
    sortState.sortMode,
    sortState.sortOrder,
    sortState.bookmarkFirst,
  ];

  const { data, isLoading, mutate } = useSWR(
    swrKey,
    fetchPlaybooksAndAssistants,
    {
      onError: (err) => {
        logger.error('Failed to load playbooks', err);
        if (onError) onError(err);
      },
    }
  );

  const playbooks = data?.playbooks || [];
  const assistants = data?.assistants || {};
  const loading = isLoading; // simplified, we can let the UI handle the rest since SWR is handling fetching

  const fetchData = useCallback(async () => {
    await mutate();
  }, [mutate]);

  const handleBookmarkToggle = async (
    id: string,
    isBookmarked: boolean,
    agentId: string,
  ) => {
    try {
      // Optimistic update
      await mutate(
        (prevData) => {
          if (!prevData) return prevData;
          return {
            ...prevData,
            playbooks: prevData.playbooks.map((p) =>
              p.id === id ? { ...p, isBookmarked } : p
            ),
          };
        },
        { revalidate: false }
      );

      await togglePlaybookBookmark(id, isBookmarked, agentId);
    } catch (error) {
      logger.error('Failed to toggle bookmark', error);
      // Revert on failure
      await mutate();
      throw error;
    }
  };

  const confirmDelete = async (playbookToDelete: PlaybookWithMeta) => {
    if (!playbookToDelete) return;
    setIsDeleting(true);

    try {
      await deletePlaybook(playbookToDelete.id, playbookToDelete.agentId);
      await mutate(
        (prevData) => {
          if (!prevData) return prevData;
          return {
            ...prevData,
            playbooks: prevData.playbooks.filter(
              (p) => p.id !== playbookToDelete.id
            ),
          };
        },
        { revalidate: false }
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
