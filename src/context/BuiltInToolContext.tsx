import { useMCPServer } from '@/hooks/use-mcp-server';
import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { MCPConfig, ToolCall } from '@/models/chat';
import { useRustBackend } from '@/hooks/use-rust-backend';
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

// Local MCPTool type that includes a required execute function accepting
// a Record<string, unknown> for stronger type safety when calling local tools.
type LocalMCPTool = MCPTool & {
  execute: (args: Record<string, unknown>) => Promise<unknown>;
};

// Built-in MCP 설정을 JSON에서 로드
const BUILTIN_MCP_CONFIGS: MCPConfig = {
  mcpServers: builtinMCPServers.servers,
};

interface BuiltInToolContextType {
  availableTools: MCPTool[];
  isLoadingTauriTools: boolean;
  // Accept LocalMCPTool[] so callers must provide execute implementations.
  registerLocalTools: (tools: LocalMCPTool[]) => void;
  executeToolCall: (toolCall: ToolCall) => Promise<MCPResponse>;
}

interface BuiltInToolProviderProps {
  children: ReactNode;
}

const BuiltInToolContext = createContext<BuiltInToolContextType | null>(null);

function BuiltInToolProviderInternal({ children }: BuiltInToolProviderProps) {
  const { connectServers, executeToolCall: executeRemoteTool } = useMCPServer();
  const { serverStates, getServerTools, executeCall } = useWebMCPTools();
  const { listBuiltinTools, callBuiltinTool } = useRustBackend();

  // Tauri built-in MCP 서버 상태 관리
  const [tauriBuiltinTools, setTauriBuiltinTools] = useState<MCPTool[]>([]);
  const [isLoadingTauriTools, setIsLoadingTauriTools] = useState(false);

  // Local frontend tools state management (store LocalMCPTool so execute is typed)
  const [localTools, setLocalTools] = useState<LocalMCPTool[]>([]);

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
        const tools = await listBuiltinTools();
        // Add builtin. prefix to Tauri built-in tools
        const prefixedTauriTools = tools.map((tool) => ({
          ...tool,
          name: tool.name.startsWith('builtin.')
            ? tool.name
            : `builtin.${tool.name}`,
        }));

        logger.info('Loaded Tauri built-in tools with builtin. prefix', {
          toolCount: prefixedTauriTools.length,
          toolNames: prefixedTauriTools.map((t) => t.name),
        });
        setTauriBuiltinTools(prefixedTauriTools);
      } catch (error) {
        logger.error('Failed to load Tauri built-in tools', error);
        setTauriBuiltinTools([]);
      } finally {
        setIsLoadingTauriTools(false);
      }
    };

    loadTauriBuiltinTools();
  }, []);

  // Register local tools function with builtin. namespace prefix and duplicate checking
  const registerLocalTools = useMemo(
    () => (newTools: LocalMCPTool[]) => {
      logger.debug('Registering local tools', {
        toolCount: newTools.length,
        toolNames: newTools.map((t) => t.name),
      });

      // Add builtin. prefix to tool names and check for duplicates
      const prefixedTools = newTools.map((tool) => ({
        ...tool,
        name: tool.name.startsWith('builtin.')
          ? tool.name
          : `builtin.${tool.name}`,
      })) as LocalMCPTool[];

      setLocalTools((prev) => {
        // Check for duplicate names within the same namespace
        const existingNames = new Set(prev.map((t) => t.name));
        const duplicates = prefixedTools.filter((tool) =>
          existingNames.has(tool.name),
        );

        if (duplicates.length > 0) {
          const duplicateNames = duplicates.map((t) => t.name);
          logger.error('Duplicate tool names detected in builtin namespace', {
            duplicateNames,
          });
          throw new Error(
            `Duplicate builtin tool names detected: ${duplicateNames.join(', ')}`,
          );
        }

        logger.info('Local tools registered with builtin. prefix', {
          toolCount: prefixedTools.length,
          toolNames: prefixedTools.map((t) => t.name),
        });

        return [...prev, ...prefixedTools];
      });
    },
    [],
  );

  // 통합된 tool 실행 함수
  const executeToolCall = useMemo(
    () =>
      async (toolCall: ToolCall): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;

        try {
          const args = toolCall.function.arguments;
          // toolCall.function.arguments may arrive as a JSON string (from MCP) or already as an object.
          // Parse if it's a non-empty string; otherwise pass through. Use try/catch to avoid crashing on invalid JSON.
          let parsedArgs: unknown = args;
          if (typeof args === 'string') {
            if (args.length === 0) {
              parsedArgs = undefined;
            } else {
              try {
                parsedArgs = JSON.parse(args);
              } catch (parseError) {
                logger.warn(
                  'Failed to parse toolCall.function.arguments as JSON, falling back to raw string',
                  {
                    toolName,
                    rawArguments: args,
                    parseError,
                  },
                );
                // keep parsedArgs as the raw string so callers can decide how to handle it
                parsedArgs = args;
              }
            }
          }

          // Check if this is a local tool (with builtin. prefix)
          const localTool = localTools.find((tool) => tool.name === toolName);
          if (localTool) {
            logger.debug('Executing local tool', { toolName, parsedArgs });
            try {
              // Ensure parsedArgs is an object; otherwise pass an empty object.
              const localArgs =
                typeof parsedArgs === 'object' && parsedArgs !== null
                  ? (parsedArgs as Record<string, unknown>)
                  : {};
              const result = await localTool.execute(localArgs);
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
            } catch (localError) {
              logger.error('Local tool execution failed', {
                toolName,
                error: localError,
              });
              return {
                jsonrpc: '2.0',
                id: toolCall.id,
                error: {
                  code: -32603,
                  message:
                    localError instanceof Error
                      ? localError.message
                      : String(localError),
                  data: { toolName, originalError: String(localError) },
                },
              };
            }
          }

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
                parsedArgs,
              });

              const tauriArgs = (parsedArgs ?? {}) as Record<string, unknown>;
              const result = await callBuiltinTool(
                serverName,
                actualToolName,
                tauriArgs,
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
            logger.debug('Executing Web Worker tool', { toolName, parsedArgs });
            const workerArgs = (parsedArgs ?? {}) as Record<string, unknown>;
            const result = await executeCall(toolName, workerArgs);
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
    [
      localTools,
      tauriBuiltinTools,
      webWorkerMcptools,
      executeCall,
      executeRemoteTool,
    ],
  );

  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools: [
        ...webWorkerMcptools,
        ...tauriBuiltinTools,
        ...localTools,
      ],
      isLoadingTauriTools,
      registerLocalTools,
      executeToolCall,
    }),
    [
      webWorkerMcptools,
      tauriBuiltinTools,
      localTools,
      isLoadingTauriTools,
      registerLocalTools,
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
