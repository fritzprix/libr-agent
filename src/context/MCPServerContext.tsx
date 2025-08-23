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
import {
  MCPResponse,
  MCPTool,
  normalizeToolResult,
  SamplingOptions,
  SamplingResponse,
} from '../lib/mcp-types';
import { MCPConfig } from '../models/chat';

const logger = getLogger('MCPServerContext');

export interface MCPServerContextType {
  availableTools: MCPTool[];
  getAvailableTools: () => MCPTool[];
  isLoading: boolean;
  error?: string;
  status: Record<string, boolean>;
  connectServers: (mcpConfigs: MCPConfig) => Promise<void>;
  executeToolCall: (toolCall: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }) => Promise<MCPResponse>;
  sampleFromModel: (
    serverName: string,
    prompt: string,
    options?: SamplingOptions,
  ) => Promise<SamplingResponse>;
}

export const MCPServerContext = createContext<MCPServerContextType | undefined>(
  undefined,
);

export const MCPServerProvider: React.FC<{ children: ReactNode }> = ({
  children,
}) => {
  const [availableTools, setAvailableTools] = useState<MCPTool[]>([]);
  const [serverStatus, setServerStatus] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | undefined>(undefined);
  const availableToolsRef = useRef(availableTools);

  const [{ loading: isLoading }, connectServers] = useAsyncFn(
    async (mcpConfig: MCPConfig) => {
      const serverStatus: Record<string, boolean> = {};
      setError(undefined);
      try {
        if (!mcpConfig.mcpServers) {
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
        const rawTools = await tauriMCPClient.listToolsFromConfig(mcpConfig);

        // Add external. prefix to all external MCP tools
        const tools = rawTools.map((tool) => ({
          ...tool,
          name: tool.name
        }));

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
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        setError(errorMessage);
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

      // Handle external. prefix for namespace routing

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

  const sampleFromModel = useCallback(
    async (
      serverName: string,
      prompt: string,
      options?: SamplingOptions,
    ): Promise<SamplingResponse> => {
      logger.debug('Context: Sampling from model', {
        serverName,
        prompt,
        options,
      });
      return tauriMCPClient.sampleFromModel(serverName, prompt, options);
    },
    [],
  );

  useEffect(() => {
    availableToolsRef.current = availableTools;
  }, [availableTools]);

  const getAvailableTools = useCallback(() => {
    return availableToolsRef.current;
  }, []);

  const value: MCPServerContextType = useMemo(
    () => ({
      availableTools,
      isLoading,
      error,
      getAvailableTools,
      status: serverStatus,
      connectServers,
      executeToolCall,
      sampleFromModel,
    }),
    [
      availableTools,
      isLoading,
      error,
      serverStatus,
      getAvailableTools,
      connectServers,
      executeToolCall,
      sampleFromModel,
    ],
  );

  return (
    <MCPServerContext.Provider value={value}>
      {children}
    </MCPServerContext.Provider>
  );
};
