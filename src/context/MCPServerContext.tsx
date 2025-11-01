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
import { useRustBackend } from '../hooks/use-rust-backend';
import {
  MCPResponse,
  MCPTool,
  SamplingOptions,
  SamplingResponse,
  MCPServerConfigV2,
} from '../lib/mcp-types';
import { MCPConfig, ToolCall, Assistant } from '../models/chat';
import { toValidJsName } from '@/lib/utils';
import { dbUtils } from '@/lib/db/service';

const logger = getLogger('MCPServerContext');

export interface MCPServerContextType {
  availableTools: MCPTool[];
  getAvailableTools: () => MCPTool[];
  isLoading: boolean;
  error?: string;
  status: Record<string, boolean>;
  connectServers: (mcpConfigs: MCPConfig) => Promise<void>;
  connectServersFromAssistant: (assistant: Assistant) => Promise<void>;
  executeToolCall: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
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
  const toolsByServer = useRef<Record<string, MCPTool[]>>({});
  const aliasToIdTableRef = useRef<Map<string, string>>(new Map());
  const [serverStatus, setServerStatus] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | undefined>(undefined);
  const availableToolsRef = useRef(availableTools);
  const {
    listToolsFromConfig,
    getConnectedServers,
    callMCPTool,
    sampleFromModel: rustSampleFromModel,
  } = useRustBackend();

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
          toolsByServer.current = {};
          return;
        }

        Object.keys(mcpConfig.mcpServers).forEach((name) => {
          serverStatus[name] = false;
        });

        setServerStatus(serverStatus);
        const rawToolsByServer = await listToolsFromConfig(mcpConfig);
        toolsByServer.current = rawToolsByServer;

        const availableTools: MCPTool[] = Object.entries(
          rawToolsByServer,
        ).flatMap(([s, tools]) => {
          if (!aliasToIdTableRef.current.has(s)) {
            aliasToIdTableRef.current.set(toValidJsName(s), s);
          }
          return tools.map((t) => ({
            ...t,
            name: `${toValidJsName(s)}__${t.name}`,
          }));
        });

        setAvailableTools(availableTools);

        const connectedServers = await getConnectedServers();
        for (const serverName of connectedServers) {
          if (Object.prototype.hasOwnProperty.call(serverStatus, serverName)) {
            serverStatus[serverName] = true;
          }
        }
        setServerStatus({ ...serverStatus });
        logger.debug(
          `Total tools loaded: ${availableTools.length} across ${Object.keys(rawToolsByServer).length} servers`,
        );
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        setError(errorMessage);
        logger.error('Error connecting to MCP:', { error });
        Object.keys(serverStatus).forEach((key) => {
          serverStatus[key] = false;
        });
        aliasToIdTableRef.current.clear();
        setServerStatus({ ...serverStatus });
        toolsByServer.current = {};
      }
    },
    [],
  );

  /**
   * Connects MCP servers from an Assistant configuration
   * Loads server entities by IDs, filters active ones, and converts to MCPConfig
   */
  const connectServersFromAssistant = useCallback(
    async (assistant: Assistant) => {
      setError(undefined);

      if (!assistant.mcpServerIds || assistant.mcpServerIds.length === 0) {
        logger.debug('No MCP servers configured for assistant');
        setAvailableTools([]);
        setServerStatus({});
        return;
      }

      try {
        // 1. DB에서 서버 엔티티 조회
        const entities = await dbUtils.getMCPServersByIds(
          assistant.mcpServerIds,
        );

        if (entities.length === 0) {
          logger.warn('No MCP server entities found for the given IDs');
          setAvailableTools([]);
          setServerStatus({});
          return;
        }

        // 2. 활성화된 서버만 필터링
        const activeEntities = entities.filter((e) => e.isActive);

        if (activeEntities.length === 0) {
          logger.info('All configured MCP servers are inactive');
          setAvailableTools([]);
          setServerStatus({});
          return;
        }

        // 3. MCPConfig 구성 (DB metadata 제외)
        const mcpConfig: MCPConfig = {
          mcpServers: Object.fromEntries(
            activeEntities.map((entity) => [
              entity.name,
              {
                name: entity.name,
                transport: entity.transport,
                authentication: entity.authentication,
                metadata: entity.metadata,
              } as MCPServerConfigV2,
            ]),
          ),
        };

        logger.debug(
          `Connecting ${activeEntities.length} active MCP servers from assistant "${assistant.name}"`,
        );

        // 4. 기존 connectServers 로직 재사용
        await connectServers(mcpConfig);
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        setError(errorMessage);
        logger.error('Error connecting servers from assistant:', { error });
        setAvailableTools([]);
        setServerStatus({});
      }
    },
    [connectServers],
  );

  const executeToolCall = useCallback(
    async (toolCall: ToolCall): Promise<MCPResponse<unknown>> => {
      logger.debug(`Executing tool call:`, { toolCall });
      const aiProvidedToolName = toolCall.function.name;

      // Handle external. prefix for namespace routing

      const delimiter = '__';
      const parts = aiProvidedToolName.split(delimiter);
      const alias = parts.length > 1 ? parts[0] : undefined;
      const toolName =
        parts.length > 1 ? parts.slice(1).join(delimiter) : aiProvidedToolName;

      if (alias && toolName) {
        const serverName = aliasToIdTableRef.current.get(alias);

        if (!serverName || !toolName) {
          const errorMsg = `Could not determine server/tool name from '${aiProvidedToolName}'`;
          logger.error(errorMsg);
          return {
            jsonrpc: '2.0',
            id: aiProvidedToolName,
            error: {
              code: -32601,
              message: errorMsg,
            },
          };
        }

        const toolArguments: Record<string, unknown> = JSON.parse(
          toolCall.function.arguments,
        );

        try {
          const rawResponse = await callMCPTool(
            serverName,
            toolName,
            toolArguments,
          );

          logger.info(`MCP Response for ${aiProvidedToolName}:`, {
            rawResponse,
          });

          // Rust backend already returns standard MCPResponse format
          // No normalization needed - this preserves UI resources
          return rawResponse;
        } catch (execError) {
          const errorMsg = `Tool execution failed: ${execError instanceof Error ? execError.message : String(execError)}`;
          logger.error(errorMsg, { execError });
          return {
            jsonrpc: '2.0',
            id: aiProvidedToolName,
            error: {
              code: -32603,
              message: errorMsg,
            },
          };
        }
      } else {
        throw new Error(
          `Tool name format invalid, missing '__' delimiter: ${aiProvidedToolName}`,
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
      return rustSampleFromModel(serverName, prompt, options);
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
      connectServersFromAssistant,
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
      connectServersFromAssistant,
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
