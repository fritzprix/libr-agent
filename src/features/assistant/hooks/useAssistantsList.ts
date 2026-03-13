import { useState, useEffect, useCallback, useRef } from 'react';
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

  const lastSearchQueryRef = useRef<string>('');

  // Load builtin tool display names
  useEffect(() => {
    let active = true;
    async function loadDefinitions() {
      try {
        const defs = await listAvailableBuiltinServerDefinitions();
        if (!active) return;
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
    return () => {
      active = false;
    };
  }, []);

  // Load MCP server names for all assistants
  useEffect(() => {
    let active = true;
    async function loadMcpServers() {
      try {
        const allMcpServerIds = new Set<string>();
        assistants.forEach((assistant) => {
          assistant.mcpServerIds?.forEach((id) => allMcpServerIds.add(id));
        });

        if (allMcpServerIds.size === 0) {
          if (active) setMcpServersMap({});
          return;
        }

        const serverIds = Array.from(allMcpServerIds);
        const entities = await dbUtils.getMCPServersByIds(serverIds);

        if (!active) return;

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
    return () => {
      active = false;
    };
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
      lastSearchQueryRef.current = query;

      if (!query.trim()) {
        setSearchResults(null);
        return;
      }

      setIsSearching(true);
      try {
        const results = await searchAssistants(query);
        // Only update if this is still the latest query
        if (lastSearchQueryRef.current === query) {
          setSearchResults(results);
        }
      } catch (error) {
        logger.error('Search failed', error);
        if (lastSearchQueryRef.current === query) {
          setSearchResults([]);
        }
      } finally {
        if (lastSearchQueryRef.current === query) {
          setIsSearching(false);
        }
      }
    },
    [searchAssistants]
  );

  const handleClearSearch = useCallback(() => {
    setSearchQuery('');
    lastSearchQueryRef.current = '';
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
