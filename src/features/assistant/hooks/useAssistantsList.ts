import { useState, useCallback, useMemo, useEffect, useRef } from 'react';
import useSWR, { useSWRConfig } from 'swr';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import { dbUtils } from '@/lib/db/service';
import { useAssistantContext } from '@/context/AssistantContext';
import { useDebouncedValue } from '@/features/knowledge/hooks/useDebouncedValue';
import type { Assistant } from '@/models/chat';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAssistantsList');

const EXIT_MS = 220;

function collectMcpServerIds(list: Assistant[], into: Set<string>) {
  for (const assistant of list) {
    for (const id of assistant.mcpServerIds ?? []) {
      into.add(id);
    }
  }
}

function isSearchAssistantsKey(key: unknown): key is [string, string] {
  return Array.isArray(key) && key[0] === 'search-assistants';
}

export function useAssistantsList() {
  const { assistants, searchAssistants, deleteAssistant } =
    useAssistantContext();
  const { mutate } = useSWRConfig();

  const [createNew, setCreateNew] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [exitingIds, setExitingIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [removedIds, setRemovedIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const inFlightDeletesRef = useRef<Set<string>>(new Set());

  const { data: builtinToolsMap = {} } = useSWR(
    'builtin-tool-definitions',
    async () => {
      const defs = await listAvailableBuiltinServerDefinitions();
      const map: Record<string, string> = {};
      defs.forEach((d) => {
        map[d.name] = d.metadata.displayName;
      });
      return map;
    },
    { revalidateOnFocus: false },
  );

  const debouncedSearchQuery = useDebouncedValue(searchQuery, 300);

  const { data: searchResults = null, isValidating: isSearching } = useSWR(
    debouncedSearchQuery.trim()
      ? ['search-assistants', debouncedSearchQuery.trim()]
      : null,
    async ([, query]: [string, string]) => {
      return await searchAssistants(query);
    },
    {
      revalidateOnFocus: false,
      shouldRetryOnError: false,
      keepPreviousData: true,
    },
  );

  const isDebouncing = searchQuery !== debouncedSearchQuery;
  const finalIsSearching =
    searchQuery.trim() !== '' && (isDebouncing || isSearching);
  const finalSearchResults =
    !isDebouncing && debouncedSearchQuery.trim() ? searchResults : null;

  const allMcpServerIds = useMemo(() => {
    const ids = new Set<string>();
    collectMcpServerIds(assistants, ids);
    if (finalSearchResults) {
      collectMcpServerIds(finalSearchResults, ids);
    }
    return Array.from(ids).sort();
  }, [assistants, finalSearchResults]);

  const { data: mcpServersMap = {} } = useSWR(
    allMcpServerIds.length > 0 ? ['mcp-servers-map', allMcpServerIds] : null,
    async ([, ids]) => {
      const entities = await dbUtils.getMCPServersByIds(ids as string[]);
      const map: Record<string, string> = {};
      entities.forEach((e) => {
        map[e.id] = e.name;
      });
      return map;
    },
    { revalidateOnFocus: false, keepPreviousData: true },
  );

  const displayedAssistants = useMemo(() => {
    const source = finalSearchResults ?? assistants;
    if (removedIds.size === 0) return source;
    return source.filter((a) => !removedIds.has(a.id));
  }, [finalSearchResults, assistants, removedIds]);

  // Clear optimistic ids once the server list / search cache no longer has them
  useEffect(() => {
    if (removedIds.size === 0) return;
    const sourceIds = new Set(
      (finalSearchResults ?? assistants).map((a) => a.id),
    );
    let changed = false;
    const next = new Set<string>();
    for (const id of removedIds) {
      if (sourceIds.has(id)) {
        next.add(id);
      } else {
        changed = true;
      }
    }
    if (changed) {
      setRemovedIds(next);
    }
  }, [removedIds, finalSearchResults, assistants]);

  const handleToggleExpand = useCallback((id: string) => {
    setExpandedId((prev) => (prev === id ? null : id));
  }, []);

  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query);
  }, []);

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  const removeFromSearchCache = useCallback(
    (assistantId: string) => {
      void mutate(
        isSearchAssistantsKey,
        (current: Assistant[] | undefined) =>
          current?.filter((a) => a.id !== assistantId),
        { revalidate: false },
      );
    },
    [mutate],
  );

  /**
   * Optimistic delete: exit animation → remove from UI → persist.
   * Caller should pass preserveScroll to keep viewport position.
   */
  const handleDeleteAssistant = useCallback(
    async (assistantId: string, options?: { preserveScroll?: () => void }) => {
      if (!assistantId || inFlightDeletesRef.current.has(assistantId)) {
        return;
      }
      inFlightDeletesRef.current.add(assistantId);

      setExitingIds((prev) => new Set(prev).add(assistantId));
      options?.preserveScroll?.();

      await new Promise<void>((resolve) => {
        window.setTimeout(resolve, EXIT_MS);
      });

      setRemovedIds((prev) => new Set(prev).add(assistantId));
      setExitingIds((prev) => {
        const next = new Set(prev);
        next.delete(assistantId);
        return next;
      });
      setExpandedId((prev) => (prev === assistantId ? null : prev));
      removeFromSearchCache(assistantId);
      options?.preserveScroll?.();

      try {
        await deleteAssistant(assistantId);
        options?.preserveScroll?.();
      } catch (error) {
        logger.error('Optimistic delete failed, restoring card', error);
        setRemovedIds((prev) => {
          const next = new Set(prev);
          next.delete(assistantId);
          return next;
        });
        void mutate(isSearchAssistantsKey);
        options?.preserveScroll?.();
        throw error;
      } finally {
        inFlightDeletesRef.current.delete(assistantId);
      }
    },
    [deleteAssistant, removeFromSearchCache, mutate],
  );

  return {
    createNew,
    setCreateNew,
    searchQuery,
    searchResults: finalSearchResults,
    displayedAssistants,
    isSearching: finalIsSearching,
    expandedId,
    exitingIds,
    builtinToolsMap,
    mcpServersMap,
    handleToggleExpand,
    handleSearch,
    handleClearSearch,
    handleDeleteAssistant,
  };
}
