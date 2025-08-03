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

const logger = getLogger('AssistantContext');

const DEFAULT_PROMPT =
  "You are an AI assistant agent that can use external tools via MCP (Model Context Protocol).\n- Always analyze the user's intent and, if needed, use available tools to provide the best answer.\n- When a tool is required, call the appropriate tool with correct parameters.\n- If the answer can be given without a tool, respond directly.\n- Be concise and clear. If you use a tool, explain the result to the user in natural language.\n- If you are unsure, ask clarifying questions before taking action.";

interface AssistantContextType {
  assistants: Assistant[];
  currentAssistant: Assistant | null;
  getCurrentAssistant: () => Assistant | null;
  setCurrentAssistant: (assistant: Assistant | null) => void;
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
  const [error, setError] = useState<Error | null>(null);
  // Helper to show user-friendly error messages
  const showError = useCallback((message: string, errorObj?: unknown) => {
    logger.error(message, { error: errorObj });
    toast.error(message);
  }, []);
  const currentAssistantRef = useRef(currentAssistant);

  const [{ value: assistants, loading, error: loadError }, loadAssistants] =
    useAsyncFn(async () => {
      let fetchedAssistants = await dbService.assistants.getPage(0, -1);
      logger.info('fetched assistants : ', { fetchedAssistants });
      return fetchedAssistants.items;
    }, []);

  // ephemeral이 우선권을 갖도록 id 기준으로 중복 제거
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

  // assistant가 전환될 때 toast로 이름 안내
  useEffect(() => {
    if (currentAssistant) {
      toast(`Assistant 전환: ${currentAssistant.name}`);
    }
  }, [currentAssistant]);

  const [{ error: saveError }, upsertAssistant] = useAsyncFn(
    async (editingAssistant: Assistant): Promise<Assistant | undefined> => {
      if (!editingAssistant?.name) {
        showError('이름은 필수입니다.');
        return;
      }

      // 기본 systemPrompt가 없으면 Agent에 맞는 프롬프트로 자동 설정
      const systemPrompt = editingAssistant.systemPrompt || DEFAULT_PROMPT;

      try {
        // 기존 Assistant 편집 시 id를 유지, 새 Assistant일 때만 id 생성

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

        logger.info(`Saving assistant`, { assistantToSave });

        await dbService.assistants.upsert(assistantToSave);

        if (currentAssistant?.id === assistantToSave.id || !currentAssistant) {
          setCurrentAssistant(assistantToSave);
        }
        await loadAssistants();
        return assistantToSave;
      } catch (err) {
        showError('어시스턴트 저장 중 오류가 발생했습니다.', err);
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
          `정말로 '${assistantName}' 어시스턴트를 삭제하시겠습니까? 이 작업은 되돌릴 수 없습니다.`,
        )
      ) {
        try {
          await dbService.assistants.delete(assistantId);
        } catch (err) {
          showError('어시스턴트 삭제 중 오류가 발생했습니다.', err);
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

  useEffect(() => {
    if (!loading && assistants && !currentAssistant) {
      if (assistants.length === 0) {
        // 사용 가능한 어시스턴트가 없으면 기본 어시스턴트를 자동 생성 및 저장합니다.
        const a = getDefaultAssistant();
        setCurrentAssistant(a);
        upsertAssistant(a);
      } else {
        logger.info('assistants : ', { assistants });
        const a = assistants.find((a) => a.isDefault) || assistants[0];
        setCurrentAssistant(a);
      }
    }
  }, [loading, assistants, upsertAssistant]);

  const getCurrentAssistant = useCallback(() => {
    return currentAssistantRef.current;
  }, []);

  logger.info('assistant context : ', {
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
    [], // No dependency on assistants!
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

  const contextValue: AssistantContextType = useMemo(
    () => ({
      assistants: allAssistants,
      currentAssistant,
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
      currentAssistant,
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
