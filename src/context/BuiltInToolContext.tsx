import { useMCPServer } from '@/hooks/use-mcp-server';
import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { MCPConfig } from '@/models/chat';
import { tauriMCPClient } from '@/lib/tauri-mcp-client';
import { getLogger } from '@/lib/logger';
import {
  createContext,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { WebMCPProvider } from './WebMCPContext';
import builtinMCPServers from '@/config/builtin-mcp-servers.json';
import { BuiltInToolsSystemPrompt } from '@/features/prompts/BuiltInToolsSystemPrompt';

const logger = getLogger('BuiltInToolContext');

// Built-in MCP 설정을 JSON에서 로드
const BUILTIN_MCP_CONFIGS: MCPConfig = {
  mcpServers: builtinMCPServers.servers,
};

interface BuiltInToolContextType {
  availableTools: MCPTool[];
  webWorkerTools: MCPTool[];
  tauriBuiltinTools: MCPTool[];
  isLoadingTauriTools: boolean;
  executeToolCall: (toolCall: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }) => Promise<MCPResponse>;
}

interface BuiltInToolProviderProps {
  children: ReactNode;
}

const BuiltInToolContext = createContext<BuiltInToolContextType | null>(null);

function BuiltInToolProviderInternal({ children }: BuiltInToolProviderProps) {
  const { connectServers, executeToolCall: executeRemoteTool } = useMCPServer();
  const { serverStates, getServerTools, executeCall } = useWebMCPTools();

  // Tauri built-in MCP 서버 상태 관리
  const [tauriBuiltinTools, setTauriBuiltinTools] = useState<MCPTool[]>([]);
  const [isLoadingTauriTools, setIsLoadingTauriTools] = useState(false);

  const servers: string[] = useMemo(
    () => Object.keys(serverStates),
    [serverStates],
  );
  const webWorkerMcptools = useMemo(
    () => servers.flatMap((s) => getServerTools(s)),
    [getServerTools, servers],
  );

  useEffect(() => {
    connectServers(BUILTIN_MCP_CONFIGS);
  }, [connectServers]);

  // Tauri built-in 도구 로드
  useEffect(() => {
    const loadTauriBuiltinTools = async () => {
      try {
        setIsLoadingTauriTools(true);
        logger.debug('Loading Tauri built-in tools...');
        const tools = await tauriMCPClient.listBuiltinTools();
        logger.info('Loaded Tauri built-in tools', { 
          toolCount: tools.length,
          toolNames: tools.map(t => t.name)
        });
        setTauriBuiltinTools(tools);
      } catch (error) {
        logger.error('Failed to load Tauri built-in tools', error);
        setTauriBuiltinTools([]);
      } finally {
        setIsLoadingTauriTools(false);
      }
    };

    loadTauriBuiltinTools();
  }, []);

  // 통합된 tool 실행 함수
  const executeToolCall = useMemo(
    () =>
      async (toolCall: {
        id: string;
        type: 'function';
        function: { name: string; arguments: string };
      }): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;

        try {
          const args = JSON.parse(toolCall.function.arguments);

          // Tauri built-in 서버 도구인지 확인 (builtin.로 시작)
          if (toolName.startsWith('builtin.')) {
            // Tauri built-in 도구 실행
            logger.debug('Detected Tauri built-in tool', { toolName });

            // builtin.filesystem__list_directory → server: "builtin.filesystem", tool: "list_directory"
            const parts = toolName.split('__');
            if (parts.length >= 2) {
              const serverName = parts[0]; // "builtin.filesystem"
              const actualToolName = parts.slice(1).join('__'); // "list_directory"
              
              logger.debug('Parsed builtin tool call', { 
                originalToolName: toolName,
                serverName, 
                actualToolName,
                args 
              });

              const result = await tauriMCPClient.callBuiltinTool(
                serverName,
                actualToolName,
                args,
              );

              logger.debug('Tauri built-in tool execution completed', {
                toolName,
                serverName,
                actualToolName,
                success: !result.error,
              });

              return {
                ...result,
                id: toolCall.id,
              };
            } else {
              const errorMsg = `Invalid builtin tool name format: ${toolName}`;
              logger.error(errorMsg);
              return {
                jsonrpc: '2.0',
                id: toolCall.id,
                error: {
                  code: -32602,
                  message: errorMsg,
                  data: { toolName },
                },
              };
            }
          }

          // Web Worker 도구인지 확인 (server__tool 형식)
          const isWebWorkerTool = webWorkerMcptools.some(
            (tool) => tool.name === toolName,
          );

          if (isWebWorkerTool) {
            // Web Worker 도구 실행
            logger.debug('Executing Web Worker tool', { toolName, args });
            const result = await executeCall(toolName, args);
            return {
              jsonrpc: '2.0',
              id: toolCall.id,
              result: {
                content: [
                  {
                    type: 'text',
                    text:
                      typeof result === 'string'
                        ? result
                        : JSON.stringify(result, null, 2),
                  },
                ],
              },
            };
          }

          // Remote MCP 도구 실행
          logger.debug('Executing remote MCP tool', { toolName, args });
          return executeRemoteTool(toolCall);
        } catch (error) {
          logger.error('Tool execution failed', { toolName, error });
          return {
            jsonrpc: '2.0',
            id: toolCall.id,
            error: {
              code: -32603,
              message: error instanceof Error ? error.message : String(error),
              data: { toolName, originalError: String(error) },
            },
          };
        }
      },
    [tauriBuiltinTools, webWorkerMcptools, executeCall, executeRemoteTool],
  );

  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools: [...webWorkerMcptools, ...tauriBuiltinTools],
      webWorkerTools: webWorkerMcptools,
      tauriBuiltinTools,
      isLoadingTauriTools,
      executeToolCall,
    }),
    [
      webWorkerMcptools,
      tauriBuiltinTools,
      isLoadingTauriTools,
      executeToolCall,
    ],
  );

  return (
    <BuiltInToolContext.Provider value={contextValue}>
      {children}
    </BuiltInToolContext.Provider>
  );
}

/**
 * Built-in tools context provider that integrates both Web Worker MCP tools
 * and remote MCP server tools into a unified interface.
 */
function BuiltInToolProvider({ children }: BuiltInToolProviderProps) {
  return (
    <WebMCPProvider servers={['content-store']} autoLoad={true}>
      <BuiltInToolProviderInternal>{children}</BuiltInToolProviderInternal>
    </WebMCPProvider>
  );
}

/**
 * Built-in 도구들을 사용하기 위한 hook
 */
function useBuiltInTools() {
  const context = useContext(BuiltInToolContext);
  if (!context) {
    throw new Error(
      'useBuiltInTools must be used within a BuiltInToolProvider',
    );
  }
  return context;
}

export { BuiltInToolProvider, useBuiltInTools, BuiltInToolsSystemPrompt };
