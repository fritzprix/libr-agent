import { useState, useEffect, useCallback, useMemo } from 'react';
import useSWR from 'swr';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import { dbUtils } from '@/lib/db/service';
import { useAssistantContext } from '@/context/AssistantContext';

export function useAssistantsList() {
  const { assistants, searchAssistants, setPaginationMode } =
    useAssistantContext();

  const [createNew, setCreateNew] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedId, setExpandedId] = useState<string | null>(null);

  // Load builtin tool display names using SWR
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

  // Load MCP server names using SWR
  const allMcpServerIds = useMemo(() => {
    const ids = new Set<string>();
    assistants.forEach((a) => a.mcpServerIds?.forEach((id) => ids.add(id)));
    return Array.from(ids).sort();
  }, [assistants]);

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
    { revalidateOnFocus: false },
  );

  // Search results using SWR to prevent race conditions
  const { data: searchResults = null, isValidating: isSearching } = useSWR(
    searchQuery.trim() ? ['search-assistants', searchQuery.trim()] : null,
    async ([, query]) => {
      return await searchAssistants(query as string);
    },
    { revalidateOnFocus: false, shouldRetryOnError: false },
  );

  const handleToggleExpand = useCallback((id: string) => {
    setExpandedId((prev) => (prev === id ? null : id));
  }, []);

  // Enable paginated mode on mount
  useEffect(() => {
    setPaginationMode('paginated');
    return () => {
      setPaginationMode('full');
    };
  }, [setPaginationMode]);

  const handleSearch = useCallback((query: string) => {
    setSearchQuery(query);
  }, []);

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
  }, []);

  return {
    createNew,
    setCreateNew,
    searchQuery,
    searchResults,
    isSearching,
    expandedId,
    builtinToolsMap,
    mcpServersMap,
    handleToggleExpand,
    handleSearch,
    handleClearSearch,
  };
}
