import React, {
  createContext,
  ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { useAsyncFn } from 'react-use';
import { getLogger } from '../lib/logger';
import { tauriMCPClient } from '../lib/tauri-mcp-client';
import { MCPResponse, MCPTool, normalizeToolResult } from '../lib/mcp-types';
import { MCPConfig } from '../models/chat';
import { useScheduledCallback } from '@/hooks/use-scheduled-callback';

const logger = getLogger('MCPServerContext');

interface MCPServerContextType {
  availableTools: MCPTool[];
  getAvailableTools: () => MCPTool[];
  isConnecting: boolean;
  status: Record<string, boolean>;
  connectServers: (mcpConfigs: MCPConfig) => Promise<void>;
  executeToolCall: (toolCall: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }) => Promise<MCPResponse>;
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
  const [{ loading: isConnecting }, connectServers] = useAsyncFn(
    async (mcpConfig: MCPConfig) => {
      const serverStatus: Record<string, boolean> = {};
      try {
        if (!mcpConfig.mcpServers) {
          // TODO: put logging
          return;
        }

        const servers = Object.keys(mcpConfig.mcpServers);

        if (servers.length === 0) {
          setServerStatus({});
          setAvailableTools([]);
          return;
        }

        Object.keys(mcpConfig.mcpServers).forEach((name) => {
          serverStatus[name] = false;
        });

        setServerStatus(serverStatus);
        const tools = await tauriMCPClient.listToolsFromConfig(mcpConfig);
        logger.debug(`Received tools from Tauri:`, {
          tools,
          totalServers: servers.length,
        });

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
        logger.error('Error connecting to MCP:', { error });
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
      type: 'function';
      function: { name: string; arguments: string };
    }): Promise<MCPResponse> => {
      logger.debug(`Executing tool call:`, { toolCall });
      const aiProvidedToolName = toolCall.function.name;
      const delimiter = '__';
      const parts = aiProvidedToolName.split(delimiter);
      const serverName = parts.length > 1 ? parts[0] : undefined;
      const toolName =
        parts.length > 1 ? parts.slice(1).join(delimiter) : aiProvidedToolName;

      if (!serverName || !toolName) {
        const errorMsg = `Could not determine server/tool name from '${aiProvidedToolName}'`;
        logger.error(errorMsg);
        return normalizeToolResult(
          { error: errorMsg, success: false },
          aiProvidedToolName,
        );
      }

      let toolArguments: Record<string, unknown>;
      try {
        toolArguments = JSON.parse(toolCall.function.arguments);
      } catch (parseError) {
        const errorMsg = `Failed to parse arguments: ${parseError instanceof Error ? parseError.message : String(parseError)}`;
        logger.error(errorMsg, { arguments: toolCall.function.arguments });
        return normalizeToolResult(
          { error: errorMsg, success: false },
          aiProvidedToolName,
        );
      }

      try {
        const rawResponse = await tauriMCPClient.callTool(
          serverName,
          toolName,
          toolArguments,
        );
        logger.debug(`MCP Response for ${aiProvidedToolName}:`, {
          rawResponse,
        });

        // 응답을 normalizeToolResult로 한 번 더 검증하여 에러 패턴 감지
        const mcpResponse = normalizeToolResult(
          rawResponse,
          aiProvidedToolName,
        );
        return mcpResponse;
      } catch (execError) {
        const errorMsg = `Tool execution failed: ${execError instanceof Error ? execError.message : String(execError)}`;
        logger.error(errorMsg, { execError });
        return normalizeToolResult(
          { error: errorMsg, success: false },
          aiProvidedToolName,
        );
      }
    },
    [],
  );

  const scheduleExecuteToolCall = useScheduledCallback(executeToolCall, [
    executeToolCall,
  ]);

  useEffect(() => {
    availableToolsRef.current = availableTools;
  }, [availableTools]);

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
      executeToolCall: scheduleExecuteToolCall,
    }),
    [
      availableTools,
      isConnecting,
      serverStatus,
      getAvailableTools,
      connectServers,
      scheduleExecuteToolCall,
    ],
  );

  return (
    <MCPServerContext.Provider value={value}>
      {children}
    </MCPServerContext.Provider>
  );
};
