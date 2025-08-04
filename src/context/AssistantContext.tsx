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

const DEFAULT_PROMPT =
  "You are an AI assistant agent that can use external tools via MCP (Model Context Protocol).\n- Always analyze the user's intent and, if needed, use available tools to provide the best answer.\n- When a tool is required, call the appropriate tool with correct parameters.\n- If the answer can be given without a tool, respond directly.\n- Be concise and clear. If you use a tool, explain the result to the user in natural language.\n- If you are unsure, ask clarifying questions before taking action.";

interface AssistantContextType {
  assistants: Assistant[];
  currentAssistant: Assistant | null;
  getCurrentAssistant: () => Assistant | null;
  setCurrentAssistant: (assistant: Assistant | null) => void;
  extend : (assistant: Partial<Assistant>) => void;
  reset : () => void;
  getAssistant: (id: string) => Assistant | null;
  upsert: (assistant: Assistant) => Promise<Assistant | undefined>;
  delete: (assistantId: string) => Promise<void>;
  registerEphemeralAssistant: (assistant: Assistant) => void;
  unregisterEphemeralAssistant: (id: string) => void;
  error: Error | null;
}

const AssistantContext = createContext<AssistantContextType | undefined>(
  undefined,
);

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
    mcpConfig: DEFAULT_MCP_CONFIG,
    systemPrompt: DEFAULT_PROMPT,
    updatedAt: new Date(),
  };
}

export function getNewAssistantTemplate(): Assistant {
  return {
    name: 'New Assistant',
    systemPrompt:
      'You are a helpful AI assistant with access to various tools. Use the available tools to help users accomplish their tasks.',
    mcpConfig: DEFAULT_MCP_CONFIG,
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
  const [ephemeralAssistants, setEphemeralAssistants] = useState<Assistant[]>(
    [],
  );

  const [ extension, setExtension ] = useState<Partial<Assistant>|null>(null);
  const [error, setError] = useState<Error | null>(null);
  // Helper to show user-friendly error messages
  const showError = useCallback((message: string, errorObj?: unknown) => {
    const logger = getLogger('AssistantContext.showError');
    logger.error(message, { error: errorObj });
    toast.error(message);
  }, []);
  const currentAssistantRef = useRef(currentAssistant);

  const [{ value: assistants, loading, error: loadError }, loadAssistants] =
    useAsyncFn(async () => {
      const logger = getLogger('AssistantContext.loadAssistants');
      let fetchedAssistants = await dbService.assistants.getPage(0, -1);
      logger.info('fetched assistants : ', { fetchedAssistants });
      return fetchedAssistants.items;
    }, []);

  // Ephemeral has priority, remove duplicates by id
  const allAssistants = useMemo(() => {
    const ephemeralMap = new Map(ephemeralAssistants.map((a) => [a.id, a]));
    const dbAssistants = (assistants ?? []).filter(
      (a) => !ephemeralMap.has(a.id),
    );
    return [...ephemeralAssistants, ...dbAssistants];
  }, [assistants, ephemeralAssistants]);

  const assistantsRef = useRef(assistants);
  useEffect(() => {
    assistantsRef.current = assistants;
  }, [assistants]);

  useEffect(() => {
    if (!loading && !assistants) {
      loadAssistants();
    }
  }, [loadAssistants, loading, assistants]);

  useEffect(() => {
    currentAssistantRef.current = currentAssistant;
  }, [currentAssistant]);

  // Assistant switched toast notification in English
  useEffect(() => {
    if (currentAssistant) {
      toast(`Assistant switched: ${currentAssistant.name}`);
    }
  }, [currentAssistant]);

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
          mcpConfig: editingAssistant.mcpConfig,
          isDefault: editingAssistant.isDefault || false,
          localServices: editingAssistant.localServices || [],
          createdAt: assistantCreatedAt || new Date(),
          updatedAt: new Date(),
        };

        const logger = getLogger('AssistantContext.upsertAssistant');
        logger.info(`Saving assistant`, { assistantToSave });

        await dbService.assistants.upsert(assistantToSave);

        // Remove from ephemeral if it was saved to DB
        setEphemeralAssistants((prev) =>
          prev.filter((a) => a.id !== assistantToSave.id)
        );

        if (currentAssistant?.id === assistantToSave.id || !currentAssistant) {
          setCurrentAssistant(assistantToSave);
        }
        await loadAssistants();
        return assistantToSave;
      } catch (err) {
        showError('Failed to save assistant.', err);
        setError(
          err instanceof Error ? err : new Error('Failed to save assistant'),
        );
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
          setError(
            err instanceof Error
              ? err
              : new Error('Failed to delete assistant'),
          );
        } finally {
          await loadAssistants();
        }
      }
    },
    [loadAssistants, assistants, showError],
  );

  useEffect(() => {
    // Prioritize showing the most recent error
    if (saveError) {
      setError(saveError);
    } else if (deleteError) {
      setError(deleteError);
    } else if (loadError) {
      setError(loadError);
    }
  }, [saveError, deleteError, loadError]);

  const handledApplyAssistantExtension = useCallback((ext: Partial<Assistant>) => {
    const logger = getLogger('AssistantContext.extend');
    
    if (!ext || Object.keys(ext).length === 0) {
      logger.warn('Empty extension provided to extend function');
      return;
    }
    
    logger.info('Applying assistant extension', { 
      extension: {
        name: ext.name,
        systemPrompt: !!ext.systemPrompt,
        localServices: ext.localServices?.length || 0,
        mcpServers: Object.keys(ext.mcpConfig?.mcpServers || {}).length,
      }
    });
    
    setExtension(ext);
  }, []);

  const handleResetAssistantExtension = useCallback(() => {
    const logger = getLogger('AssistantContext.reset');
    logger.debug('Resetting assistant extension');
    setExtension(null);
  }, []);

  useEffect(() => {
    if (!loading && assistants && !currentAssistant) {
      if (assistants.length === 0) {
        // Create and save default assistant if none available
        const a = getDefaultAssistant();
        setCurrentAssistant(a);
        upsertAssistant(a);
      } else {
        const logger = getLogger('AssistantContext.initializeAssistant');
        logger.info('assistants : ', { assistants });
        const a = assistants.find((a) => a.isDefault) || assistants[0];
        setCurrentAssistant(a);
      }
    }
  }, [loading, assistants, upsertAssistant]);

  const getCurrentAssistant = useCallback(() => {
    return currentAssistantRef.current;
  }, []);

  const contextLogger = getLogger('AssistantContext.provider');
  contextLogger.info('assistant context : ', {
    assistants: assistants?.length,
    error,
  });

  const handleRegisterEphemeral = useCallback(
    (assistant: Assistant) => {
      const existsInDb = (assistantsRef.current ?? []).some(
        (a) => a.id === assistant.id,
      );
      if (existsInDb) {
        return;
      }
      setEphemeralAssistants((prev) => {
        const filtered = prev.filter((a) => a.id !== assistant.id);
        return [...filtered, assistant];
      });
    },
    [], // No dependency on assistants to prevent circular dependency
  );

  const handleUnregisterEphemeral = useCallback((id: string) => {
    setEphemeralAssistants((prev) => {
      return prev.filter((a) => a.id !== id);
    });
  }, []);

  const handleGetAssistant = useCallback(
    (id: string) => {
      return allAssistants.find((a) => a.id === id) || null;
    },
    [allAssistants],
  );

  const effectiveAssistant: Assistant | null = useMemo(() => {
    if (extension && currentAssistant) {
      const logger = getLogger('AssistantContext.effectiveAssistant');
      
      // System prompt: append extension context if available
      const basePrompt = currentAssistant.systemPrompt || '';
      const extPrompt = extension.systemPrompt;
      const combinedPrompt = extPrompt 
        ? [basePrompt, extPrompt].filter(Boolean).join('\n\n')
        : basePrompt;

      // Local services: merge existing + additional (deduplicated)
      const baseServices = currentAssistant.localServices || [];
      const extServices = extension.localServices || [];
      const combinedServices = [...new Set([...baseServices, ...extServices])];

      // MCP config: merge existing + additional servers (extension takes priority)
      const baseMcpConfig = currentAssistant.mcpConfig || { mcpServers: {} };
      const extMcpConfig = extension.mcpConfig || { mcpServers: {} };
      const combinedMcpConfig = {
        ...baseMcpConfig,
        mcpServers: {
          ...baseMcpConfig.mcpServers,
          ...extMcpConfig.mcpServers, // Extension has priority
        },
      };

      const result = {
        ...currentAssistant,
        ...extension, // Other fields allow override
        systemPrompt: combinedPrompt,
        localServices: combinedServices,
        mcpConfig: combinedMcpConfig,
      } satisfies Assistant;

      logger.debug('Applied extension to assistant', {
        baseAssistant: currentAssistant.name,
        extension: {
          systemPrompt: !!extPrompt,
          localServices: extServices.length,
          mcpServers: Object.keys(extMcpConfig.mcpServers || {}),
        },
        result: {
          systemPromptLength: result.systemPrompt.length,
          localServicesCount: result.localServices?.length,
          mcpServersCount: Object.keys(result.mcpConfig?.mcpServers || {}).length,
        },
      });

      return result;
    }
    return currentAssistant;
  }, [currentAssistant, extension]);

  const contextValue: AssistantContextType = useMemo(
    () => ({
      assistants: allAssistants,
      currentAssistant: effectiveAssistant,
      extend: handledApplyAssistantExtension,
      reset: handleResetAssistantExtension,
      setCurrentAssistant,
      unregisterEphemeralAssistant: handleUnregisterEphemeral,
      registerEphemeralAssistant: handleRegisterEphemeral,
      getAssistant: handleGetAssistant,
      getCurrentAssistant,
      upsert: upsertAssistant,
      delete: deleteAssistant,
      error: error ?? null,
    }),
    [
      allAssistants,
      effectiveAssistant,
      handledApplyAssistantExtension,
      handleResetAssistantExtension,
      setCurrentAssistant,
      getCurrentAssistant,
      upsertAssistant,
      deleteAssistant,
      error,
      handleRegisterEphemeral,
      handleUnregisterEphemeral,
      handleGetAssistant,
    ],
  );

  return (
    <AssistantContext.Provider value={contextValue}>
      {children}
    </AssistantContext.Provider>
  );
};

export function useAssistantContext() {
  const ctx = useContext(AssistantContext);
  if (!ctx)
    throw new Error(
      'useAssistantContext must be used within a AssistantContextProvider',
    );
  return ctx;
}
