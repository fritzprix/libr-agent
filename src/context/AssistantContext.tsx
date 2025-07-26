import { createId } from "@paralleldrive/cuid2";
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useAsyncFn } from "react-use";
import { dbService } from "../lib/db";
import { getLogger } from "../lib/logger";
import { Assistant } from "../types/chat";

const logger = getLogger("AssistantContext");

const DEFAULT_PROMPT =
  "You are an AI assistant agent that can use external tools via MCP (Model Context Protocol).\n- Always analyze the user's intent and, if needed, use available tools to provide the best answer.\n- When a tool is required, call the appropriate tool with correct parameters.\n- If the answer can be given without a tool, respond directly.\n- Be concise and clear. If you use a tool, explain the result to the user in natural language.\n- If you are unsure, ask clarifying questions before taking action.";

interface AssistantContextType {
  assistants: Assistant[];
  currentAssistant: Assistant | null;
  getCurrentAssistant: () => Assistant | null;
  setCurrentAssistant: (assistant: Assistant | null) => void;
  saveAssistant: (
    assistant: Partial<Assistant>,
    mcpConfigText: string,
  ) => Promise<Assistant | undefined>;
  deleteAssistant: (assistantId: string) => Promise<void>;
  getNewAssistantTemplate: () => {
    assistant: Partial<Assistant>;
    mcpConfigText: string;
  };
  error: Error | null;
}

const AssistantContext = createContext<AssistantContextType | undefined>(
  undefined,
);

export const DEFAULT_MCP_CONFIG = {
  mcpServers: {
    "sequential-thinking": {
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-sequential-thinking"],
      env: {},
    },
    filesystem: {
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
      env: {},
    },
  },
};

function getDefaultAssistant(): Assistant {
  return {
    id: createId(),
    createdAt: new Date(),
    name: "Default Assistant",
    isDefault: true,
    mcpConfig: DEFAULT_MCP_CONFIG,
    systemPrompt: DEFAULT_PROMPT,
    updatedAt: new Date(),
  };
}

function getNewAssistantTemplate(): {
  assistant: Partial<Assistant>;
  mcpConfigText: string;
} {
  const defaultMcpConfig = DEFAULT_MCP_CONFIG;

  return {
    assistant: {
      name: "",
      systemPrompt:
        "You are a helpful AI assistant with access to various tools. Use the available tools to help users accomplish their tasks.",
      mcpConfig: {},
    },
    mcpConfigText: JSON.stringify(defaultMcpConfig, null, 2),
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
  const currentAssistantRef = useRef(currentAssistant);
  const [{ value: assistants, loading, error }, loadAssistants] =
    useAsyncFn(async () => {
      let fetchedAssistants = await dbService.assistants.getPage(0, -1);
      logger.info("fetched assistants : ", { fetchedAssistants });
      return fetchedAssistants.items;
    }, []);

  useEffect(() => {
    if (!loading && !assistants) {
      loadAssistants();
    }
  }, [loadAssistants, loading, assistants]);

  useEffect(() => {
    currentAssistantRef.current = currentAssistant;
  }, [currentAssistant]);

  useEffect(() => {
    if (!loading && assistants && !currentAssistant) {
      if (assistants.length === 0) {
        const a = getDefaultAssistant();
        setCurrentAssistant(a);
        saveAssistant(a);
      } else {
        logger.info("assistants : ", { assistants });
        const a = assistants.find((a) => a.isDefault) || assistants[0];
        setCurrentAssistant(a);
      }
    }
  }, [loading, assistants]);

  const [{}, saveAssistant] = useAsyncFn(
    async (
      editingAssistant: Partial<Assistant>,
      mcpConfigText?: string,
    ): Promise<Assistant | undefined> => {
      if (!editingAssistant?.name) {
        alert("이름은 필수입니다.");
        return;
      }
      // Parse MCP config from text and validate JSON
      let mcpConfigJson = JSON.stringify(editingAssistant.mcpConfig);

      if (mcpConfigJson === mcpConfigText) {
        mcpConfigJson = mcpConfigText;
      }
      let mcpConfig;
      try {
        if (mcpConfigJson.trim()) {
          mcpConfig = JSON.parse(mcpConfigJson);
        } else {
          mcpConfig = {};
        }
      } catch (error) {
        alert("유효하지 않은 JSON 형식입니다. JSON을 확인해주세요.");
        return undefined;
      }

      // 기본 systemPrompt가 없으면 Agent에 맞는 프롬프트로 자동 설정
      const systemPrompt = editingAssistant.systemPrompt || DEFAULT_PROMPT;

      try {
        const finalConfig = { mcpServers: mcpConfig.mcpServers || {} };

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
          mcpConfig: finalConfig,
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
      } catch (error) {
        logger.error("Error saving assistant:", { error });
        alert("어시스턴트 저장 중 오류가 발생했습니다.");
        return undefined;
      }
    },
    [currentAssistant, loadAssistants],
  );

  const [{}, deleteAssistant] = useAsyncFn(
    async (assistantId: string) => {
      const assistant = assistants?.find((a) => a.id === assistantId);
      const assistantName = assistant?.name || "Unknown";
      if (
        window.confirm(
          `정말로 '${assistantName}' 어시스턴트를 삭제하시겠습니까? 이 작업은 되돌릴 수 없습니다.`,
        )
      ) {
        try {
          await dbService.assistants.delete(assistantId);
          await loadAssistants();
          // The useEffect hook will handle setting a new currentAssistant
        } catch (error) {
          logger.error("Error deleting assistant:", { error });
          alert("어시스턴트 삭제 중 오류가 발생했습니다.");
        } finally {
          await loadAssistants();
        }
      }
    },
    [loadAssistants],
  );

  const getCurrentAssistant = useCallback(() => {
    return currentAssistantRef.current;
  }, []);

  const getNewAssistantTemplateCallback = useCallback(() => {
    return getNewAssistantTemplate();
  }, []);

  logger.info("assistant context : ", {
    assistants: assistants?.length,
    error,
  });

  const contextValue: AssistantContextType = useMemo(
    () => ({
      assistants: assistants || [],
      currentAssistant,
      setCurrentAssistant,
      getCurrentAssistant,
      saveAssistant,
      deleteAssistant,
      getNewAssistantTemplate: getNewAssistantTemplateCallback,
      error: error ?? null,
    }),
    [
      assistants,
      currentAssistant,
      setCurrentAssistant,
      getCurrentAssistant,
      saveAssistant,
      deleteAssistant,
      getNewAssistantTemplateCallback,
      error,
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
      "useAssistantContext must be used within a AssistantContextProvider",
    );
  return ctx;
}
