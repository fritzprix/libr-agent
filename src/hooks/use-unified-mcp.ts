import { useMCPServer } from './use-mcp-server';
import { useWebMCPTools } from './use-web-mcp';
import { useBuiltInTools } from '@/context/BuiltInToolContext';
import { useCallback, useMemo } from 'react';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useUnifiedMCP');

interface ToolCall {
  id: string;
  type: 'function';
  function: { name: string; arguments: string };
}

type BackendType = 'tauri' | 'web' | 'builtin';

/**
 * 🔧 Unified MCP Hook
 *
 * Integrates Tauri-based MCP servers, Web Worker MCP tools, and Tauri builtin tools
 * into a single interface for tool execution.
 */
export const useUnifiedMCP = () => {
  const { executeToolCall: executeTauriTool, availableTools: tauriTools } =
    useMCPServer();
  const {
    executeCall: executeWebTool,
    availableTools: webTools,
    isInitialized: isWebReady,
  } = useWebMCPTools();
  const {
    executeToolCall: executeBuiltinTool,
    availableTools: builtinTools,
  } = useBuiltInTools();

  // Create lookup map for fast tool type resolution
  const toolTypeMap = useMemo((): Map<string, BackendType> => {
    const map = new Map<string, BackendType>();

    // Add Tauri tools
    tauriTools.forEach((tool) => {
      map.set(tool.name, 'tauri');
    });

    // Add Web Worker tools
    webTools.forEach((tool) => {
      map.set(tool.name, 'web');
    });

    // Add Builtin tools
    builtinTools.forEach((tool) => {
      map.set(tool.name, 'builtin');
    });

    logger.debug('Tool type map created', {
      tauriCount: tauriTools.length,
      webCount: webTools.length,
      builtinCount: builtinTools.length,
      totalMapped: map.size,
    });

    return map;
  }, [tauriTools, webTools, builtinTools]);

  // Combine all available tools (include builtin tools)
  const allTools = useMemo(() => {
    return [...tauriTools, ...webTools, ...builtinTools];
  }, [tauriTools, webTools, builtinTools]);

  // Fast tool type lookup using the pre-built map
  const getToolType = useCallback(
    (toolName: string): BackendType | null => {
      return toolTypeMap.get(toolName) || null;
    },
    [toolTypeMap],
  );

  // Determine if a tool is a web worker tool (optimized with lookup)
  const isWebWorkerTool = useCallback(
    (toolName: string): boolean => {
      return toolTypeMap.get(toolName) === 'web';
    },
    [toolTypeMap],
  );

  // Check if a tool exists (optimized with lookup)
  const hasTools = useCallback(
    (toolName: string): boolean => {
      return toolTypeMap.has(toolName);
    },
    [toolTypeMap],
  );

  // Unified tool execution
  const executeToolCall = useCallback(
    async (toolCall: ToolCall): Promise<MCPResponse> => {
      const toolName = toolCall.function.name;
      const toolType = getToolType(toolName);

      if (!toolType) {
        logger.warn(`Unknown tool: ${toolName}`, {
          availableTools: Array.from(toolTypeMap.keys()),
        });
        return {
          jsonrpc: '2.0',
          id: toolCall.id,
          error: {
            code: -32601,
            message: `Tool '${toolName}' not found`,
            data: {
              toolName,
              availableTools: Array.from(toolTypeMap.keys()),
            },
          },
        };
      }

      logger.info(`Executing ${toolType} tool: ${toolName}`, { toolCall });

      try {
        if (toolType === 'web') {
          // Execute web worker tool
          const args = JSON.parse(toolCall.function.arguments);
          const result = await executeWebTool(toolName, args);

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
        } else if (toolType === 'builtin') {
          // Execute Tauri builtin tool
          return await executeBuiltinTool(toolCall);
        } else {
          // Execute Tauri MCP tool
          return await executeTauriTool(toolCall);
        }
      } catch (error) {
        logger.error(`Tool execution failed for ${toolName}:`, error);

        return {
          jsonrpc: '2.0',
          id: toolCall.id,
          error: {
            code: -32603,
            message: error instanceof Error ? error.message : String(error),
            data: {
              toolName,
              toolType,
              errorType:
                error instanceof Error
                  ? error.constructor.name
                  : 'UnknownError',
            },
          },
        };
      }
    },
    [getToolType, executeWebTool, executeTauriTool, toolTypeMap],
  );

  // Get tool by name
  const getTool = useCallback(
    (toolName: string): MCPTool | null => {
      return allTools.find((tool) => tool.name === toolName) || null;
    },
    [allTools],
  );

  // Get tools by type using the lookup map
  const getToolsByType = useCallback(() => {
    const result = {
      tauri: [] as MCPTool[],
      web: [] as MCPTool[],
      builtin: [] as MCPTool[],
      all: allTools,
    };

    allTools.forEach((tool) => {
      const type = toolTypeMap.get(tool.name);
      if (type === 'tauri') {
        result.tauri.push(tool);
      } else if (type === 'web') {
        result.web.push(tool);
      } else if (type === 'builtin') {
        result.builtin.push(tool);
      }
    });

    return result;
  }, [allTools, toolTypeMap]);

  // Get tool type statistics
  const getToolStats = useCallback(() => {
    const stats: { tauri: number; web: number; builtin: number; total: number } = {
      tauri: 0,
      web: 0,
      builtin: 0,
      total: toolTypeMap.size,
    };

    toolTypeMap.forEach((type) => {
      if (type === 'tauri') stats.tauri++;
      else if (type === 'web') stats.web++;
      else if (type === 'builtin') stats.builtin++;
    });

    return stats;
  }, [toolTypeMap]);

  return {
    // Tool execution
    executeToolCall,

    // Tool information
    availableTools: allTools,
    getTool,
    hasTools,
    getToolsByType,
    getToolStats,

    // Type checking (optimized)
    getToolType,
    isWebWorkerTool,

    // Lookup map (for debugging/inspection)
    toolTypeMap,

    // Status
    isWebReady,
    toolCounts: getToolStats(),
  };
};
