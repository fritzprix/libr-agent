import React, {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import { useMCPServer } from '../hooks/use-mcp-server';
import { useWebMCPTools } from '../hooks/use-web-mcp';
import { getLogger } from '../lib/logger';
import { MCPResponse, MCPTool, normalizeToolResult } from '../lib/mcp-types';

const logger = getLogger('UnifiedMCPContext');

type ToolExecutionStrategy = 'tauri' | 'webworker' | 'local';

interface ToolExecutionContext {
  strategy: ToolExecutionStrategy;
  serverName?: string;
  originalToolName: string;
}

interface UnifiedMCPContextType {
  // Combined tools from all MCP systems
  availableTools: MCPTool[];

  // Tool operations
  executeToolCall: (toolCall: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }) => Promise<MCPResponse>;

  // Tool discovery
  findTool: (toolName: string) => MCPTool | undefined;
  getToolExecutionContext: (toolName: string) => ToolExecutionContext | null;

  // System status
  isReady: boolean;
  systemStatus: {
    tauriMCP: {
      connected: boolean;
      toolCount: number;
      status: Record<string, boolean>;
    };
    webMCP: {
      initialized: boolean;
      toolCount: number;
      error: string | null;
    };
  };
}

export const UnifiedMCPContext = createContext<
  UnifiedMCPContextType | undefined
>(undefined);

interface UnifiedMCPProviderProps {
  children: ReactNode;
}

export const UnifiedMCPProvider: React.FC<UnifiedMCPProviderProps> = ({
  children,
}) => {
  // Tauri MCP integration
  const {
    availableTools: tauriTools,
    executeToolCall: executeTauriTool,
    status: tauriStatus,
  } = useMCPServer();

  // Web Worker MCP integration
  const {
    availableTools: webMcpTools,
    executeCall: executeWebMcpTool,
    isReady: webMcpReady,
    error: webMcpError,
  } = useWebMCPTools();

  // Combined available tools
  const availableTools = useMemo(() => {
    const combined = [...tauriTools, ...webMcpTools];
    logger.debug('Combined available tools', {
      tauriCount: tauriTools.length,
      webMcpCount: webMcpTools.length,
      totalCount: combined.length,
    });
    return combined;
  }, [tauriTools, webMcpTools]);

  // Determine which execution strategy to use for a tool
  const getToolExecutionContext = useCallback(
    (toolName: string): ToolExecutionContext | null => {
      // Check if it's a Web MCP tool (prefixed with server name)
      const webMcpTool = webMcpTools.find((tool) => tool.name === toolName);
      if (webMcpTool) {
        const parts = toolName.split('__');
        return {
          strategy: 'webworker',
          serverName: parts.length > 1 ? parts[0] : undefined,
          originalToolName:
            parts.length > 1 ? parts.slice(1).join('__') : toolName,
        };
      }

      // Check if it's a Tauri MCP tool
      const tauriTool = tauriTools.find((tool) => tool.name === toolName);
      if (tauriTool) {
        const parts = toolName.split('__');
        return {
          strategy: 'tauri',
          serverName: parts.length > 1 ? parts[0] : undefined,
          originalToolName:
            parts.length > 1 ? parts.slice(1).join('__') : toolName,
        };
      }

      logger.warn('Tool not found in any MCP system', { toolName });
      return null;
    },
    [tauriTools, webMcpTools],
  );

  // Find a tool by name
  const findTool = useCallback(
    (toolName: string): MCPTool | undefined => {
      return availableTools.find((tool) => tool.name === toolName);
    },
    [availableTools],
  );

  // Unified tool execution
  const executeToolCall = useCallback(
    async (toolCall: {
      id: string;
      type: 'function';
      function: { name: string; arguments: string };
    }): Promise<MCPResponse> => {
      const toolName = toolCall.function.name;
      const executionContext = getToolExecutionContext(toolName);

      if (!executionContext) {
        const errorMsg = `Tool not found: ${toolName}`;
        logger.error(errorMsg);
        return normalizeToolResult(
          { error: errorMsg, success: false },
          toolName,
        );
      }

      logger.debug('Executing unified tool call', {
        toolName,
        strategy: executionContext.strategy,
        serverName: executionContext.serverName,
      });

      try {
        switch (executionContext.strategy) {
          case 'tauri': {
            // Execute via Tauri MCP
            return await executeTauriTool(toolCall);
          }

          case 'webworker': {
            // Execute via Web Worker MCP
            if (!executionContext.serverName) {
              throw new Error(
                'Server name required for Web MCP tool execution',
              );
            }

            const args = JSON.parse(toolCall.function.arguments);
            const result = await executeWebMcpTool(
              executionContext.serverName,
              executionContext.originalToolName,
              args,
            );

            // Convert Web MCP result to standard MCP Response
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

          default:
            throw new Error(
              `Unsupported execution strategy: ${executionContext.strategy}`,
            );
        }
      } catch (error) {
        const errorMessage =
          error instanceof Error ? error.message : String(error);
        logger.error('Unified tool execution failed', {
          toolName,
          strategy: executionContext.strategy,
          error: errorMessage,
        });

        return normalizeToolResult(
          { error: errorMessage, success: false },
          toolName,
        );
      }
    },
    [getToolExecutionContext, executeTauriTool, executeWebMcpTool],
  );

  // System readiness
  const isReady = useMemo(() => {
    const tauriReady =
      Object.keys(tauriStatus).length === 0 ||
      Object.values(tauriStatus).some((status) => status);
    return tauriReady || webMcpReady;
  }, [tauriStatus, webMcpReady]);

  // System status
  const systemStatus = useMemo(() => {
    const tauriConnectedCount =
      Object.values(tauriStatus).filter(Boolean).length;
    const tauriHasServers = Object.keys(tauriStatus).length > 0;

    return {
      tauriMCP: {
        connected: tauriHasServers && tauriConnectedCount > 0,
        toolCount: tauriTools.length,
        status: tauriStatus,
      },
      webMCP: {
        initialized: webMcpReady,
        toolCount: webMcpTools.length,
        error: webMcpError,
      },
    };
  }, [
    tauriStatus,
    tauriTools.length,
    webMcpReady,
    webMcpTools.length,
    webMcpError,
  ]);

  // Log system status changes
  useEffect(() => {
    logger.info('Unified MCP system status updated', {
      systemStatus,
      totalTools: availableTools.length,
      isReady,
    });
  }, [systemStatus, availableTools.length, isReady]);

  const value: UnifiedMCPContextType = useMemo(
    () => ({
      availableTools,
      executeToolCall,
      findTool,
      getToolExecutionContext,
      isReady,
      systemStatus,
    }),
    [
      availableTools,
      executeToolCall,
      findTool,
      getToolExecutionContext,
      isReady,
      systemStatus,
    ],
  );

  return (
    <UnifiedMCPContext.Provider value={value}>
      {children}
    </UnifiedMCPContext.Provider>
  );
};

/**
 * Hook for using the unified MCP system
 */
export const useUnifiedMCP = () => {
  const context = useContext(UnifiedMCPContext);
  if (context === undefined) {
    throw new Error('useUnifiedMCP must be used within a UnifiedMCPProvider');
  }
  return context;
};

/**
 * Hook for simplified unified MCP operations
 */
export const useUnifiedMCPTools = () => {
  const { availableTools, executeToolCall, findTool, isReady, systemStatus } =
    useUnifiedMCP();

  /**
   * Get all available tools from all MCP systems
   */
  const getAllTools = useCallback(() => availableTools, [availableTools]);

  /**
   * Execute a tool call by name with automatic system detection
   */
  const executeByName = useCallback(
    async (toolName: string, args: unknown): Promise<MCPResponse> => {
      const toolCall = {
        id: `unified-${toolName}-${Date.now()}`,
        type: 'function' as const,
        function: {
          name: toolName,
          arguments: JSON.stringify(args),
        },
      };

      return await executeToolCall(toolCall);
    },
    [executeToolCall],
  );

  /**
   * Get tools by system type
   */
  const getToolsBySystem = useCallback(
    (system: 'tauri' | 'webworker') => {
      if (system === 'tauri') {
        return availableTools.filter((tool) => {
          // Tauri tools typically don't have __ prefix or come from specific servers
          return (
            !tool.name.includes('calculator__') &&
            !tool.name.includes('filesystem__')
          );
        });
      } else {
        return availableTools.filter((tool) => {
          // Web MCP tools are prefixed with server names
          return (
            tool.name.includes('calculator__') ||
            tool.name.includes('filesystem__')
          );
        });
      }
    },
    [availableTools],
  );

  return {
    // Tool access
    availableTools,
    getAllTools,
    findTool,
    getToolsBySystem,

    // Tool execution
    executeToolCall,
    executeByName,

    // System status
    isReady,
    systemStatus,
  };
};
