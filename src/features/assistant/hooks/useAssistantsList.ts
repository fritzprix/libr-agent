import { useState, useEffect, useCallback } from 'react';
import { getLogger } from '@/lib/logger';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import { dbUtils } from '@/lib/db/service';
import { useAssistantContext } from '@/context/AssistantContext';
import type { Assistant } from '@/models/chat';

const logger = getLogger('useAssistantsList');

export function useAssistantsList() {
  const { assistants, searchAssistants, setPaginationMode } = useAssistantContext();

  const [createNew, setCreateNew] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [searchResults, setSearchResults] = useState<Assistant[] | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [builtinToolsMap, setBuiltinToolsMap] = useState<Record<string, string>>({});
  const [mcpServersMap, setMcpServersMap] = useState<Record<string, string>>({});

  // Load builtin tool display names
  useEffect(() => {
    async function loadDefinitions() {
      try {
        const defs = await listAvailableBuiltinServerDefinitions();
        const map: Record<string, string> = {};
        defs.forEach((d) => {
          map[d.name] = d.metadata.displayName;
        });
        setBuiltinToolsMap(map);
      } catch (err) {
        logger.error('Failed to load builtin definitions', err);
      }
    }
    loadDefinitions();
  }, []);

  // Load MCP server names for all assistants
  useEffect(() => {
    async function loadMcpServers() {
      try {
        const allMcpServerIds = new Set<string>();
        assistants.forEach((assistant) => {
          assistant.mcpServerIds?.forEach((id) => allMcpServerIds.add(id));
        });

        if (allMcpServerIds.size === 0) {
          return;
        }

        const serverIds = Array.from(allMcpServerIds);
        const entities = await dbUtils.getMCPServersByIds(serverIds);

        const map: Record<string, string> = {};
        entities.forEach((entity) => {
          map[entity.id] = entity.name;
        });

        setMcpServersMap(map);
      } catch (err) {
        logger.error('❌ Failed to load MCP servers', err);
      }
    }
    loadMcpServers();
  }, [assistants]);

  const handleToggleExpand = useCallback((id: string) => {
    setExpandedId((prev) => (prev === id ? null : id));
  }, []);

  // Enable paginated mode on mount
  useEffect(() => {
    setPaginationMode('paginated');
    return () => {
      // Reset to full mode when unmounting (for other components)
      setPaginationMode('full');
    };
  }, [setPaginationMode]);

  const handleSearch = useCallback(
    async (query: string) => {
      setSearchQuery(query);
      if (!query.trim()) {
        setSearchResults(null);
        return;
      }

      setIsSearching(true);
      try {
        const results = await searchAssistants(query);
        setSearchResults(results);
      } catch (error) {
        logger.error('Search failed', error);
        setSearchResults([]);
      } finally {
        setIsSearching(false);
      }
    },
    [searchAssistants]
  );

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
    setSearchResults(null);
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
