import { useMCPServer } from './use-mcp-server';
import { useWebMCPTools } from './use-web-mcp';
import { useBuiltInTools } from '@/context/BuiltInToolContext';
import { useCallback, useMemo } from 'react';
import { MCPResponse, MCPTool, MCPResourceContent } from '@/lib/mcp-types';
import { UIResource } from '@/models/chat';
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
  const { executeToolCall: executeBuiltinTool, availableTools: builtinTools } =
    useBuiltInTools();

  // Create lookup map for fast tool type resolution
  const toolTypeMap = useMemo((): Map<string, BackendType> => {
    const map = new Map<string, BackendType>();

    // Add Tauri tools
    tauriTools.forEach((tool: MCPTool) => {
      map.set(tool.name, 'tauri');
    });

    // Add Web Worker tools
    webTools.forEach((tool: MCPTool) => {
      map.set(tool.name, 'web');
    });

    // Add Builtin tools
    builtinTools.forEach((tool: MCPTool) => {
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

  // Get tool namespace and type from prefixed tool name
  const getToolNamespace = useCallback((toolName: string): string => {
    if (toolName.startsWith('builtin.')) return 'builtin';
    else return 'external';
  }, []);

  // Intelligent tool name resolution - maps LLM-called names to actual prefixed names
  const resolveToolName = useCallback(
    (calledToolName: string): string | null => {
      // 1. Check exact match first (already prefixed)
      if (toolTypeMap.has(calledToolName)) {
        return calledToolName;
      }

      // 2. Check if it's already namespaced correctly
      const namespace = getToolNamespace(calledToolName);
      if (namespace !== 'unknown') {
        return calledToolName;
      }

      // 3. Look for tools that end with the called name (LLM using base name)
      for (const [registeredName, type] of toolTypeMap.entries()) {
        if (type === 'web' || type === 'tauri') {
          // Check server__toolName pattern
          const parts = registeredName.split('__');
          if (parts.length >= 2) {
            const baseName = parts.slice(1).join('__');
            if (baseName === calledToolName) {
              logger.debug('Resolved tool name', {
                calledName: calledToolName,
                resolvedName: registeredName,
                pattern: 'server__tool',
              });
              return registeredName;
            }
          }
        }
        // For builtin tools like "builtin.filesystem__read_file"
        // should match when LLM calls "read_file" 
        if (type === 'builtin') {
          const nameWithoutPrefix = registeredName.replace(/^builtin\./, '');
          const parts = nameWithoutPrefix.split('__');
          if (parts.length >= 2) {
            const baseName = parts.slice(1).join('__');
            if (baseName === calledToolName) {
              logger.debug('Resolved tool name', {
                calledName: calledToolName,
                resolvedName: registeredName,
                pattern: 'builtin.server__tool',
              });
              return registeredName;
            }
          }
        }
      }

      // 4. No resolution found
      logger.warn('Could not resolve tool name', {
        calledName: calledToolName,
        availableTools: Array.from(toolTypeMap.keys()),
      });
      return null;
    },
    [toolTypeMap, getToolNamespace],
  );

  // Fast tool type lookup using the pre-built map and namespace prefixes
  const getToolType = useCallback(
    (toolName: string): BackendType | null => {
      const namespace = getToolNamespace(toolName);
      if (namespace === 'builtin') return 'builtin';
      if (namespace === 'external') {
        // For external tools, check if they're web worker or tauri based on the lookup map
        return toolTypeMap.get(toolName) || 'tauri'; // Default external tools to tauri
      }
      return toolTypeMap.get(toolName) || null;
    },
    [toolTypeMap, getToolNamespace],
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
      const calledToolName = toolCall.function.name;
      
      // Resolve the tool name to handle LLM calling base names instead of prefixed names
      const resolvedToolName = resolveToolName(calledToolName);
      
      if (!resolvedToolName) {
        const namespace = getToolNamespace(calledToolName);
        logger.warn(`Could not resolve tool: ${calledToolName} (namespace: ${namespace})`, {
          availableTools: Array.from(toolTypeMap.keys()),
          detectedNamespace: namespace,
        });
        return {
          jsonrpc: '2.0',
          id: toolCall.id,
          error: {
            code: -32601,
            message: `Tool '${calledToolName}' could not be resolved`,
            data: {
              calledToolName,
              namespace,
              availableTools: Array.from(toolTypeMap.keys()),
            },
          },
        };
      }

      const toolType = getToolType(resolvedToolName);
      if (!toolType) {
        logger.error(`Resolved tool name has no type mapping: ${resolvedToolName}`);
        return {
          jsonrpc: '2.0',
          id: toolCall.id,
          error: {
            code: -32602,
            message: `Resolved tool '${resolvedToolName}' has no type mapping`,
            data: { calledToolName, resolvedToolName },
          },
        };
      }

      logger.info(`Executing ${toolType} tool: ${resolvedToolName} (called as: ${calledToolName})`, { toolCall });

      // Use the resolved tool name for execution
      const actualToolCall = {
        ...toolCall,
        function: {
          ...toolCall.function,
          name: resolvedToolName,
        },
      };

      try {
        if (toolType === 'web') {
          // Execute web worker tool
          const args = JSON.parse(actualToolCall.function.arguments);
          const result = await executeWebTool(resolvedToolName, args);

          // Check if result is already MCPResponse
          if (
            typeof result === 'object' &&
            result !== null &&
            'jsonrpc' in result &&
            (result as MCPResponse).jsonrpc === '2.0'
          ) {
            return result as MCPResponse;
          }

          // Check if result is UIResource
          if (
            typeof result === 'object' &&
            result !== null &&
            'mimeType' in result &&
            typeof (result as UIResource).mimeType === 'string'
          ) {
            const uiResource = result as UIResource;
            return {
              jsonrpc: '2.0',
              id: actualToolCall.id,
              result: {
                content: [
                  {
                    type: 'resource',
                    resource: uiResource,
                  } as MCPResourceContent,
                ],
              },
            };
          }

          // Default text handling for other results
          return {
            jsonrpc: '2.0',
            id: actualToolCall.id,
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
        logger.error(`Tool execution failed for ${resolvedToolName}:`, error);

        return {
          jsonrpc: '2.0',
          id: actualToolCall.id,
          error: {
            code: -32603,
            message: error instanceof Error ? error.message : String(error),
            data: {
              calledToolName,
              resolvedToolName,
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
    [
      getToolType,
      getToolNamespace,
      executeWebTool,
      executeTauriTool,
      executeBuiltinTool,
      toolTypeMap,
    ],
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
    const stats: {
      tauri: number;
      web: number;
      builtin: number;
      total: number;
    } = {
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
    getToolNamespace,
    isWebWorkerTool,

    // Lookup map (for debugging/inspection)
    toolTypeMap,

    // Status
    isWebReady,
    toolCounts: getToolStats(),
  };
};
