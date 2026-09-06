import { createId } from '@paralleldrive/cuid2';
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { getLogger } from '../lib/logger';
import type { Assistant } from '../models/chat';
import { toast } from 'sonner';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import type { MCPTool } from '@/lib/mcp';
import { useSettings } from '@/hooks/use-settings';
import { AssistantService } from '@/lib/services/assistant-service';
import { useBackendResource } from '@/context/GlobalEventContext';
import { CORE_BUILTIN_SERVICE_ALIASES } from '@/lib/assistant/runtime-builtins';

const logger = getLogger('AssistantContext');

const DEFAULT_PROMPT =
  "You are an AI assistant agent that can use external tools via MCP (Model Context Protocol).\n- Always analyze the user's intent and, if needed, use available tools to provide the best answer.\n- When a tool is required, call the appropriate tool with correct parameters.\n- If the answer can be given without a tool, respond directly.\n- Be concise and clear. If you use a tool, explain the result to the user in natural language.\n- If you are unsure, ask clarifying questions before taking action.";

interface AssistantContextType {
  assistants: Assistant[];
  currentAssistant: Assistant | null;
  getCurrent: () => Assistant | null;
  setCurrentAssistant: (assistant: Assistant | null) => void;
  getById: (id: string) => Assistant | null;
  saveAssistant: (assistant: Assistant) => Promise<Assistant | undefined>;
  deleteAssistant: (assistantId: string) => Promise<void>;
  searchAssistants: (query: string) => Promise<Assistant[]>;
  availableTools: MCPTool[];
  error: Error | null;
  loading: boolean;
  isLoadingMore: boolean;
  hasMore: boolean;
  loadMore: () => Promise<void>;
  // Pagination support
  paginationMode: 'full' | 'paginated';
  setPaginationMode: (mode: 'full' | 'paginated') => void;
  currentPage: number;
  setPage: (page: number) => void;
  pageSize: number;
  totalAssistants: number;
}

const AssistantContext = createContext<AssistantContextType | undefined>(
  undefined,
);

/**
 * Default MCP Configuration
 *
 * Supports both V1 (Legacy) and V2 (MCP 2025-06-18 Spec) formats.
 * Both formats can be mixed in the same configuration.
 *
 * @example V1 Format (stdio only):
 * ```json
 * {
 *   "mcpServers": {
 *     "server-name": {
 *       "command": "npx",
 *       "args": ["-y", "@modelcontextprotocol/server-name"],
 *       "env": {}
 *     }
 *   }
 * }
 * ```
 *
 * @example V2 Format with HTTP:
 * ```json
 * {
 *   "mcpServers": {
 *     "http-server": {
 *       "name": "http-server",
 *       "transport": {
 *         "type": "http",
 *         "url": "https://api.example.com/mcp"
 *       }
 *     }
 *   }
 * }
 * ```
 *
 * @example V2 Format with OAuth 2.1:
 * ```json
 * {
 *   "mcpServers": {
 *     "oauth-server": {
 *       "name": "oauth-server",
 *       "transport": {
 *         "type": "http",
 *         "url": "https://api.example.com/mcp"
 *       },
 *       "authentication": {
 *         "type": "oauth2.1",
 *         "clientId": "your-client-id",
 *         "redirectUri": "libr-agent://oauth/callback",
 *         "scopes": ["read", "write"],
 *         "usePKCE": true,
 *         "discoveryUrl": "https://auth.example.com/.well-known/oauth-authorization-server"
 *       }
 *     }
 *   }
 * }
 * ```
 */
export const DEFAULT_MCP_CONFIG = {
  mcpServers: {
    'sequential-thinking': {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-sequential-thinking'],
      env: {},
    },
    filesystem: {
      command: 'npx',
      args: ['-y', '@modelcontextprotocol/server-filesystem', '/tmp'],
      env: {},
    },
  },
};

export function getNewAssistantTemplate(): Assistant {
  return {
    // Empty until save — AssistantEditor uses falsy id for "Create" title,
    // and upsertAssistant issues a real id via createId() when missing.
    id: '',
    name: 'New Assistant',
    systemPrompt:
      'You are a helpful AI assistant with access to various tools. Use the available tools to help users accomplish their tasks.',
    mcpServerIds: [], // No servers by default - user selects from Settings
    allowedBuiltInServiceAliases: [...CORE_BUILTIN_SERVICE_ALIASES],
    createdAt: new Date(),
    updatedAt: new Date(),
    deletionProtected: false,
  };
}

export const AssistantContextProvider = ({
  children,
  initialPaginationMode = 'full',
}: {
  children: ReactNode;
  initialPaginationMode?: 'full' | 'paginated';
}) => {
  const [currentAssistant, setCurrentAssistant] = useState<Assistant | null>(
    null,
  );

  // Pagination / infinite-scroll state
  const [paginationMode, setPaginationMode] = useState<'full' | 'paginated'>(
    initialPaginationMode,
  );
  const [currentPage, setCurrentPage] = useState(1);
  const [totalAssistants, setTotalAssistants] = useState(0);
  const [assistants, setAssistants] = useState<Assistant[]>([]);
  const [loading, setLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [loadError, setLoadError] = useState<Error | null>(null);
  const pageSize = 20;
  const loadGenerationRef = useRef(0);
  const loadMoreInFlightRef = useRef(false);

  const { connectServersFromAssistant, availableTools } = useMCPServer();
  const { activeServers } = useMCPServerRegistry();
  const { value: settings } = useSettings();

  const assistantService = useMemo(() => {
    return new AssistantService(settings.agentHubUrl);
  }, [settings.agentHubUrl]);

  const showError = useCallback((message: string, errorObj?: unknown) => {
    logger.error(message, { error: errorObj });
    toast.error(message);
  }, []);

  const currentAssistantRef = useRef(currentAssistant);

  const hasMore =
    paginationMode === 'paginated' && assistants.length < totalAssistants;

  const refreshAssistants = useCallback(async () => {
    const generation = ++loadGenerationRef.current;
    loadMoreInFlightRef.current = false;
    setIsLoadingMore(false);
    setLoading(true);
    setLoadError(null);
    try {
      if (paginationMode === 'paginated') {
        const result = await assistantService.getList({
          page: 1,
          pageSize,
        });
        if (generation !== loadGenerationRef.current) return;
        setAssistants(result.items);
        setTotalAssistants(result.totalItems);
        setCurrentPage(1);
        logger.debug('fetched assistants (page 1):', {
          items: result.items.length,
          total: result.totalItems,
        });
      } else {
        const result = await assistantService.getList({
          page: 1,
          pageSize: 1000,
        });
        if (generation !== loadGenerationRef.current) return;
        setAssistants(result.items);
        setTotalAssistants(result.totalItems);
        setCurrentPage(1);
        logger.debug('fetched assistants (full mode):', {
          count: result.items.length,
          total: result.totalItems,
        });
        if (result.totalItems > 1000) {
          logger.warn(
            'Total assistants exceed 1000, consider implementing multi-page loading or switching to paginated mode',
          );
        }
      }
    } catch (err) {
      if (generation !== loadGenerationRef.current) return;
      const error = err instanceof Error ? err : new Error(String(err));
      setLoadError(error);
      showError('Failed to load assistants.', err);
    } finally {
      if (generation === loadGenerationRef.current) {
        setLoading(false);
      }
    }
  }, [assistantService, paginationMode, pageSize, showError]);

  const loadMore = useCallback(async () => {
    if (
      paginationMode !== 'paginated' ||
      loading ||
      loadMoreInFlightRef.current ||
      assistants.length >= totalAssistants
    ) {
      return;
    }

    const nextPage = currentPage + 1;
    const generation = loadGenerationRef.current;
    loadMoreInFlightRef.current = true;
    setIsLoadingMore(true);
    try {
      const result = await assistantService.getList({
        page: nextPage,
        pageSize,
      });
      if (generation !== loadGenerationRef.current) return;

      setAssistants((prev) => {
        const seen = new Set(prev.map((a) => a.id));
        const appended = result.items.filter((item) => !seen.has(item.id));
        return [...prev, ...appended];
      });
      setTotalAssistants(result.totalItems);
      setCurrentPage(nextPage);
      logger.debug('fetched assistants (load more):', {
        items: result.items.length,
        total: result.totalItems,
        page: nextPage,
      });
    } catch (err) {
      if (generation !== loadGenerationRef.current) return;
      showError('Failed to load more assistants.', err);
    } finally {
      loadMoreInFlightRef.current = false;
      if (generation === loadGenerationRef.current) {
        setIsLoadingMore(false);
      }
    }
  }, [
    paginationMode,
    loading,
    assistants.length,
    totalAssistants,
    currentPage,
    assistantService,
    pageSize,
    showError,
  ]);

  // Reload when mode/service changes (and on mount)
  useEffect(() => {
    void refreshAssistants();
  }, [refreshAssistants]);

  const assistantsRef = useRef(assistants);
  useEffect(() => {
    assistantsRef.current = assistants;
  }, [assistants]);

  // Track previous assistant ID to prevent toast on initial load
  const prevAssistantIdRef = useRef<string | null>(null);

  // Assistant switched toast notification - only on manual switch
  useEffect(() => {
    if (currentAssistant?.id) {
      // Only show toast if this is a real switch (not initial load)
      if (
        prevAssistantIdRef.current !== null &&
        prevAssistantIdRef.current !== currentAssistant.id
      ) {
        toast(`Assistant switched: ${currentAssistant.name}`);
      }
      prevAssistantIdRef.current = currentAssistant.id;
    }
  }, [currentAssistant?.id, currentAssistant?.name]);

  const [{ error: saveError }, upsertAssistant] = useAsyncFn(
    async (editingAssistant: Assistant): Promise<Assistant | undefined> => {
      if (!editingAssistant?.name) {
        showError('Assistant name is required.');
        return;
      }

      // Set default systemPrompt if none provided
      const systemPrompt = editingAssistant.systemPrompt || DEFAULT_PROMPT;

      try {
        // Keep existing id for updates, generate new id for new assistants
        let assistantId = editingAssistant.id;
        let assistantCreatedAt = editingAssistant.createdAt;
        if (!assistantId) {
          assistantId = createId();
          assistantCreatedAt = new Date();
        }

        const assistantToSave: Assistant = {
          id: assistantId,
          name: editingAssistant.name,
          description: editingAssistant.description,
          avatar: editingAssistant.avatar,
          systemPrompt,
          mcpServerIds: editingAssistant.mcpServerIds,
          deletionProtected: editingAssistant.deletionProtected ?? false,
          localServices: editingAssistant.localServices ?? [],
          disabledSkills: editingAssistant.disabledSkills,
          createdAt: assistantCreatedAt || new Date(),
          updatedAt: new Date(),
        };

        if (editingAssistant.allowedBuiltInServiceAliases !== undefined) {
          assistantToSave.allowedBuiltInServiceAliases =
            editingAssistant.allowedBuiltInServiceAliases;
        }

        logger.info(`Saving assistant`, { assistantToSave });

        await assistantService.save(assistantToSave);

        if (currentAssistant?.id === assistantToSave.id || !currentAssistant) {
          setCurrentAssistant(assistantToSave);
        }
        await refreshAssistants();
        return assistantToSave;
      } catch (err) {
        showError('Failed to save assistant.', err);
        // Error is automatically captured by useAsyncFn's saveError
        return undefined;
      }
    },
    [currentAssistant, refreshAssistants, showError, assistantService],
  );

  const [{ error: deleteError }, deleteAssistant] = useAsyncFn(
    async (assistantId: string) => {
      try {
        await assistantService.delete(assistantId);
        if (currentAssistantRef.current?.id === assistantId) {
          setCurrentAssistant(null);
        }
        // Soft-remove so infinite-scroll position / loaded pages stay intact
        setAssistants((prev) => prev.filter((a) => a.id !== assistantId));
        setTotalAssistants((prev) => Math.max(0, prev - 1));
      } catch (err) {
        showError('Failed to delete assistant.', err);
        throw err; // Re-throw for caller to handle
      }
    },
    [showError, assistantService],
  );

  const searchAssistants = useCallback(
    async (query: string): Promise<Assistant[]> => {
      try {
        return await assistantService.search(query);
      } catch (err) {
        showError('Failed to search assistants.', err);
        return [];
      }
    },
    [assistantService, showError],
  );

  // Consolidate errors from all async operations using useMemo
  // Prioritize: saveError > deleteError > loadError
  const error = useMemo<Error | null>(() => {
    return saveError || deleteError || loadError || null;
  }, [saveError, deleteError, loadError]);

  useEffect(() => {
    if (!loading && assistants.length > 0 && !currentAssistant) {
      // Select the first assistant by default (no implicit default)
      const a = assistants[0];
      if (a) {
        setCurrentAssistant(a);
      }
    }
  }, [loading, assistants, currentAssistant]);

  const getCurrent = useCallback(() => {
    return currentAssistantRef.current;
  }, []);

  const getById = useCallback(
    (id: string) => {
      return assistants.find((a) => a.id === id) || null;
    },
    [assistants],
  );

  // Debounce MCP server reconnection to avoid rapid successive calls
  const debouncedConnectRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );

  useEffect(() => {
    currentAssistantRef.current = currentAssistant;

    // Clear any pending connection attempt
    if (debouncedConnectRef.current) {
      clearTimeout(debouncedConnectRef.current);
    }

    if (currentAssistant) {
      // Debounce connection by 500ms to avoid rapid reconnections
      debouncedConnectRef.current = setTimeout(() => {
        connectServersFromAssistant(currentAssistant);
      }, 500);
    }

    return () => {
      if (debouncedConnectRef.current) {
        clearTimeout(debouncedConnectRef.current);
      }
    };
  }, [currentAssistant, connectServersFromAssistant]);

  // React state-driven reconnection when MCP servers change (no window events)
  useEffect(() => {
    const current = currentAssistantRef.current;

    // Clear any pending connection attempt
    if (debouncedConnectRef.current) {
      clearTimeout(debouncedConnectRef.current);
    }

    if (current) {
      logger.debug('MCP servers changed, reconnecting for current assistant');
      // Debounce connection by 500ms
      debouncedConnectRef.current = setTimeout(() => {
        connectServersFromAssistant(current);
      }, 500);
    }

    return () => {
      if (debouncedConnectRef.current) {
        clearTimeout(debouncedConnectRef.current);
      }
    };
    // We intentionally depend on activeServers reference to reflect registry changes
  }, [activeServers, connectServersFromAssistant]);

  // Subscribe to local service events (Main Thread changes)
  useEffect(() => {
    const unsubscribe = assistantService.onRevalidate((event) => {
      logger.debug('Local assistant service changed, refreshing...', event);
      void refreshAssistants();
    });
    return unsubscribe;
  }, [assistantService, refreshAssistants]);

  // Subscribe to agent:event for AI agent resource updates via centralized hook
  useBackendResource('assistant', () => {
    logger.debug('Agent updated assistant resource, refreshing assistants...');
    void refreshAssistants();
  });

  const contextValue: AssistantContextType = useMemo(
    () => ({
      assistants,
      currentAssistant,
      setCurrentAssistant,
      getById,
      getCurrent,
      saveAssistant: upsertAssistant,
      deleteAssistant: deleteAssistant,
      searchAssistants,
      error: error ?? null,
      loading,
      isLoadingMore,
      hasMore,
      loadMore,
      availableTools,
      paginationMode,
      setPaginationMode,
      currentPage,
      setPage: setCurrentPage,
      pageSize,
      totalAssistants,
    }),
    [
      assistants,
      currentAssistant,
      setCurrentAssistant,
      getCurrent,
      upsertAssistant,
      deleteAssistant,
      searchAssistants,
      error,
      loading,
      isLoadingMore,
      hasMore,
      loadMore,
      getById,
      availableTools,
      paginationMode,
      setPaginationMode,
      currentPage,
      setCurrentPage,
      pageSize,
      totalAssistants,
    ],
  );

  return (
    <AssistantContext.Provider value={contextValue}>
      {children}
    </AssistantContext.Provider>
  );
};

export function useAssistantContext() {
  const context = useContext(AssistantContext);
  if (!context)
    throw new Error(
      'useAssistantContext must be used within a AssistantContextProvider',
    );
  return context;
}
