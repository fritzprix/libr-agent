import React, {
  createContext,
  ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useAsyncFn } from "react-use";
import { getLogger } from "../lib/logger";
import { MCPTool, tauriMCPClient } from "../lib/tauri-mcp-client";
import { useAssistantContext } from "./AssistantContext";
import { Assistant } from "../types/chat";

const logger = getLogger("MCPServerContext");

interface MCPServerContextType {
  availableTools: MCPTool[];
  getAvailableTools: () => MCPTool[];
  isConnecting: boolean;
  status: Record<string, boolean>;
  connectServers: (assistant: Assistant) => Promise<void>;
  executeToolCall: (toolCall: {
    id: string;
    type: "function";
    function: { name: string; arguments: string };
  }) => Promise<{ role: "tool"; content: string; tool_call_id: string }>;
}

export const MCPServerContext = createContext<MCPServerContextType | undefined>(
  undefined,
);

export const MCPServerProvider: React.FC<{ children: ReactNode }> = ({
  children,
}) => {
  const [availableTools, setAvailableTools] = useState<MCPTool[]>([]);
  const [serverStatus, setServerStatus] = useState<Record<string, boolean>>({});
  const availableToolsRef = useRef(availableTools);
  const { currentAssistant } = useAssistantContext();
  const [{ loading: isConnecting }, connectServers] = useAsyncFn(
    async (assistant: Assistant) => {
      const serverStatus: Record<string, boolean> = {};
      try {
        const configForTauri = {
          mcpServers: assistant.mcpConfig.mcpServers || {},
        };

        const servers = Object.keys(configForTauri.mcpServers);

        if (servers.length === 0) {
          setServerStatus({});
          setAvailableTools([]);
          return;
        }

        Object.keys(configForTauri.mcpServers).forEach((name) => {
          serverStatus[name] = false;
        });

        setServerStatus(serverStatus);
        const tools = await tauriMCPClient.listToolsFromConfig(configForTauri);
        logger.debug(`Received tools from Tauri:`, { tools });

        const connectedServers = await tauriMCPClient.getConnectedServers();
        for (const serverName of connectedServers) {
          if (Object.prototype.hasOwnProperty.call(serverStatus, serverName)) {
            serverStatus[serverName] = true;
          }
        }
        setServerStatus({ ...serverStatus });
        setAvailableTools(tools);
        logger.debug(`Total tools loaded: ${tools.length}`);
      } catch (error) {
        logger.error("Error connecting to MCP:", { error });
        Object.keys(serverStatus).forEach((key) => {
          serverStatus[key] = false;
        });
        setServerStatus({ ...serverStatus });
      }
    },
    [],
  );

  const executeToolCall = useCallback(
    async (toolCall: {
      id: string;
      type: "function";
      function: { name: string; arguments: string };
    }): Promise<{ role: "tool"; content: string; tool_call_id: string }> => {
      logger.debug(`Executing tool call:`, { toolCall });
      const aiProvidedToolName = toolCall.function.name;
      let serverName: string | undefined;
      let toolName: string | undefined;

      // Use '__' as delimiter for URL and JSON safety
      const delimiter = "__";
      const parts = aiProvidedToolName.split(delimiter);
      if (parts.length >= 2) {
        serverName = parts[0];
        toolName = parts.slice(1).join(delimiter);
      }

      if (!serverName || !toolName) {
        logger.error(
          `Could not determine serverName or toolName for AI-provided tool name: ${aiProvidedToolName}`,
        );
        return {
          role: "tool",
          content: `Error: Could not find tool '${aiProvidedToolName}' or determine its server.`,
          tool_call_id: toolCall.id,
        };
      }

      let toolArguments: Record<string, unknown> = {};
      try {
        toolArguments = JSON.parse(toolCall.function.arguments);
      } catch (parseError) {
        logger.error(
          `Failed to parse tool arguments for ${toolCall.function.name}:`,
          { parseError },
        );
        return {
          role: "tool",
          content: `Error: Invalid tool arguments JSON: ${parseError instanceof Error ? parseError.message : String(parseError)}`,
          tool_call_id: toolCall.id,
        };
      }

      try {
        const result = await tauriMCPClient.callTool(
          serverName,
          toolName,
          toolArguments,
        );
        logger.debug(`Tool execution result for ${toolCall.function.name}:`, {
          result,
        });
        return {
          role: "tool",
          content: JSON.stringify(result),
          tool_call_id: toolCall.id,
        };
      } catch (execError) {
        logger.error(`Tool execution failed for ${toolCall.function.name}:`, {
          execError,
        });
        return {
          role: "tool",
          content: `Error: Tool '${toolCall.function.name}' failed: ${execError instanceof Error ? execError.message : String(execError)}`,
          tool_call_id: toolCall.id,
        };
      }
    },
    [],
  );

  useEffect(() => {
    availableToolsRef.current = availableTools;
  }, [availableTools]);

  useEffect(() => {
    if (currentAssistant) {
      logger.info("connect : ", { currentAssistant: currentAssistant.name });
      connectServers(currentAssistant);
    }
  }, [connectServers, currentAssistant]);

  const getAvailableTools = useCallback(() => {
    return availableToolsRef.current;
  }, []);

  const value: MCPServerContextType = useMemo(
    () => ({
      availableTools,
      isConnecting,
      getAvailableTools,
      status: serverStatus,
      connectServers,
      executeToolCall,
    }),
    [
      availableTools,
      isConnecting,
      serverStatus,
      getAvailableTools,
      connectServers,
      executeToolCall,
    ],
  );

  return (
    <MCPServerContext.Provider value={value}>
      {children}
    </MCPServerContext.Provider>
  );
};
