import { useMCPServer } from '@/hooks/use-mcp-server';
import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { MCPConfig } from '@/models/chat';
import {
  createContext,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import { WebMCPProvider } from './WebMCPContext';
import builtinMCPServers from '@/config/builtin-mcp-servers.json';

// Built-in MCP 설정을 JSON에서 로드
const BUILTIN_MCP_CONFIGS: MCPConfig[] = [
  {
    mcpServers: builtinMCPServers.servers,
  },
];

interface BuiltInToolContextType {
  availableTools: MCPTool[];
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

  // 통합된 tool 실행 함수
  const executeToolCall = useMemo(
    () =>
      async (toolCall: {
        id: string;
        type: 'function';
        function: { name: string; arguments: string };
      }): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;

        // Web Worker 도구인지 확인 (server__tool 형식)
        const isWebWorkerTool = webWorkerMcptools.some(
          (tool) => tool.name === toolName,
        );

        if (isWebWorkerTool) {
          // Web Worker 도구 실행
          try {
            const args = JSON.parse(toolCall.function.arguments);
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
          } catch (error) {
            return {
              jsonrpc: '2.0',
              id: toolCall.id,
              error: {
                code: -32603,
                message: error instanceof Error ? error.message : String(error),
              },
            };
          }
        }

        // Remote MCP 도구 실행
        return executeRemoteTool(toolCall);
      },
    [webWorkerMcptools, executeCall, executeRemoteTool],
  );

  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools: [...webWorkerMcptools],
      executeToolCall,
    }),
    [webWorkerMcptools, executeToolCall],
  );

  return (
    <BuiltInToolContext.Provider value={contextValue}>
      {children}
    </BuiltInToolContext.Provider>
  );
}

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

export { BuiltInToolProvider, useBuiltInTools };
