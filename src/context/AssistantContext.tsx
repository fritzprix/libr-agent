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
import { dbService } from '../lib/db';
import { getLogger } from '../lib/logger';
import { Assistant } from '../models/chat';
import { toast } from 'sonner';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useMCPServerRegistry } from '@/context/MCPServerRegistryContext';
import { MCPTool } from '@/lib/mcp-types';

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
  availableTools: MCPTool[];
  error: Error | null;
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

export function getDefaultAssistant(): Assistant {
  return {
    createdAt: new Date(),
    name: 'Default Assistant',
    isDefault: true,
    mcpServerIds: [], // No servers by default - user selects from Settings
    systemPrompt: DEFAULT_PROMPT,
    updatedAt: new Date(),
  };
}

export function getNewAssistantTemplate(): Assistant {
  return {
    name: 'New Assistant',
    systemPrompt:
      'You are a helpful AI assistant with access to various tools. Use the available tools to help users accomplish their tasks.',
    mcpServerIds: [], // No servers by default - user selects from Settings
    createdAt: new Date(),
    updatedAt: new Date(),
    isDefault: false,
  };
}

export const AssistantContextProvider = ({
  children,
}: {
  children: ReactNode;
}) => {
  const [currentAssistant, setCurrentAssistant] = useState<Assistant | null>(
    null,
  );

  const { connectServersFromAssistant, availableTools } = useMCPServer();
  const { activeServers } = useMCPServerRegistry();

  // Error state is derived from async operations errors - no longer needs setState
  // const [error, setError] = useState<Error | null>(null);

  // Helper to show user-friendly error messages
  const showError = useCallback((message: string, errorObj?: unknown) => {
    logger.error(message, { error: errorObj });
    toast.error(message);
  }, []);

  const currentAssistantRef = useRef(currentAssistant);

  const [{ value: assistants, loading, error: loadError }, loadAssistants] =
    useAsyncFn(async () => {
      let fetchedAssistants = await dbService.assistants.getPage(0, -1);
      logger.debug('fetched assistants : ', { fetchedAssistants });
      return fetchedAssistants.items;
    }, []);

  // Return assistants from DB
  const allAssistants = useMemo(() => {
    return assistants ?? [];
  }, [assistants]);

  const assistantsRef = useRef(assistants);
  useEffect(() => {
    assistantsRef.current = assistants;
  }, [assistants]);

  useEffect(() => {
    if (!loading && !assistants) {
      loadAssistants();
    }
  }, [loadAssistants, loading, assistants]);

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
          systemPrompt,
          mcpServerIds: editingAssistant.mcpServerIds,
          isDefault: editingAssistant.isDefault ?? false,
          localServices: editingAssistant.localServices ?? [],
          createdAt: assistantCreatedAt || new Date(),
          updatedAt: new Date(),
        };

        if (editingAssistant.allowedBuiltInServiceAliases !== undefined) {
          assistantToSave.allowedBuiltInServiceAliases =
            editingAssistant.allowedBuiltInServiceAliases;
        }

        logger.info(`Saving assistant`, { assistantToSave });

        await dbService.assistants.upsert(assistantToSave);

        if (currentAssistant?.id === assistantToSave.id || !currentAssistant) {
          setCurrentAssistant(assistantToSave);
        }
        await loadAssistants();
        return assistantToSave;
      } catch (err) {
        showError('Failed to save assistant.', err);
        // Error is automatically captured by useAsyncFn's saveError
        return undefined;
      }
    },
    [currentAssistant, loadAssistants, showError],
  );

  const [{ error: deleteError }, deleteAssistant] = useAsyncFn(
    async (assistantId: string) => {
      const assistant = assistants?.find((a) => a.id === assistantId);
      const assistantName = assistant?.name || 'Unknown';
      if (
        window.confirm(
          `Are you sure you want to delete '${assistantName}' assistant? This action cannot be undone.`,
        )
      ) {
        try {
          await dbService.assistants.delete(assistantId);
        } catch (err) {
          showError('Failed to delete assistant.', err);
          // Error is automatically captured by useAsyncFn's deleteError
        } finally {
          await loadAssistants();
        }
      }
    },
    [loadAssistants, assistants, showError],
  );

  // Consolidate errors from all async operations using useMemo
  // Prioritize: saveError > deleteError > loadError
  const error = useMemo<Error | null>(() => {
    return saveError || deleteError || loadError || null;
  }, [saveError, deleteError, loadError]);

  useEffect(() => {
    if (!loading && assistants && !currentAssistant) {
      if (assistants.length === 0) {
        // Create and save default assistant if none available
        const a = getDefaultAssistant();
        setCurrentAssistant(a);
        upsertAssistant(a);
      } else {
        const a = assistants.find((a) => a.isDefault) || assistants[0];
        setCurrentAssistant(a);
      }
    }
  }, [loading, assistants, currentAssistant, upsertAssistant]);

  const getCurrent = useCallback(() => {
    return currentAssistantRef.current;
  }, []);

  const getById = useCallback(
    (id: string) => {
      return allAssistants.find((a) => a.id === id) || null;
    },
    [allAssistants],
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

  const contextValue: AssistantContextType = useMemo(
    () => ({
      assistants: allAssistants,
      currentAssistant,
      setCurrentAssistant,
      getById,
      getCurrent,
      saveAssistant: upsertAssistant,
      deleteAssistant: deleteAssistant,
      error: error ?? null,
      availableTools,
    }),
    [
      allAssistants,
      currentAssistant,
      setCurrentAssistant,
      getCurrent,
      upsertAssistant,
      deleteAssistant,
      error,
      getById,
      availableTools,
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
