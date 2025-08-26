import { useMCPServer } from './use-mcp-server';
// import { useWebMCPTools } from './use-web-mcp';
import { useBuiltInTool } from '@/features/tools';
import { useCallback, useMemo } from 'react';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { ToolCall } from '@/models/chat';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useUnifiedMCP');

type BackendType = 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';

export const useUnifiedMCP = () => {
  const { executeToolCall: executeExternalMCP, availableTools: externalTools } =
    useMCPServer();
  // Web MCP tools are disabled for now
  const webBuiltInTool: MCPTool[] = [];
  const isWebReady = false;
  const {
    executeTool: executeRustBuiltinTool,
    availableTools: rustBuiltinTools,
  } = useBuiltInTool();

  // Create lookup map for fast tool type resolution
  const toolTypeMap = useMemo((): Map<string, BackendType> => {
    const map = new Map<string, BackendType>();

    // Add external tools
    externalTools.forEach((tool: MCPTool) => {
      map.set(tool.name, 'ExternalMCP');
    });

    // Add Web Worker tools
    webBuiltInTool.forEach((tool: MCPTool) => {
      map.set(tool.name, 'BuiltInWeb');
    });

    // Add Builtin tools
    rustBuiltinTools.forEach((tool: MCPTool) => {
      map.set(tool.name, 'BuiltInRust');
    });

    logger.debug('Tool type map created', {
      externalTools: externalTools.length,
      webworkerBuiltinCount: webBuiltInTool.length,
      rustBuiltinCount: rustBuiltinTools.length,
      totalMapped: map.size,
    });

    return map;
  }, [externalTools, webBuiltInTool, rustBuiltinTools]);

  // Combine all available tools (include builtin tools)
  const allTools = useMemo(() => {
    return [...externalTools, ...webBuiltInTool, ...rustBuiltinTools];
  }, [externalTools, webBuiltInTool, rustBuiltinTools]);

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

      // 2. Look for tools that end with the called name (LLM using base name)
      for (const [registeredName, type] of toolTypeMap.entries()) {
        if (type === 'ExternalMCP') {
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
        if (type === 'BuiltInWeb' || type === 'BuiltInRust') {
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

  const getToolType = useCallback(
    (toolName: string): BackendType | null => {
      // Prefer direct lookup from the pre-built map
      const direct = toolTypeMap.get(toolName);
      if (direct) return direct;

      // Fallback: infer builtin by prefix, otherwise unknown
      if (!toolName.startsWith('builtin.')) return 'ExternalMCP';

      return null;
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
        logger.warn(
          `Could not resolve tool: ${calledToolName} (namespace: ${namespace})`,
          {
            availableTools: Array.from(toolTypeMap.keys()),
            detectedNamespace: namespace,
          },
        );
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
        logger.error(
          `Resolved tool name has no type mapping: ${resolvedToolName}`,
        );
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

      logger.info(
        `Executing ${toolType} tool: ${resolvedToolName} (called as: ${calledToolName})`,
        { toolCall },
      );

      // Use the resolved tool name for execution
      const actualToolCall = {
        ...toolCall,
        function: {
          ...toolCall.function,
          name: resolvedToolName,
        },
      };

      try {
        if (toolType === 'BuiltInWeb') {
          throw new Error('Web MCP tools are currently disabled');
        } else if (toolType === 'BuiltInRust') {
          return await executeRustBuiltinTool(actualToolCall);
        } else {
          return await executeExternalMCP(actualToolCall);
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
      executeExternalMCP,
      executeRustBuiltinTool,
      // executeWebBuiltinTool, // disabled
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
      externalMcpServer: [] as MCPTool[],
      webBuiltIn: [] as MCPTool[],
      rustBuiltIn: [] as MCPTool[],
      all: allTools,
    };

    allTools.forEach((tool) => {
      const type = toolTypeMap.get(tool.name);
      if (type === 'ExternalMCP') {
        result.externalMcpServer.push(tool);
      } else if (type === 'BuiltInWeb') {
        result.webBuiltIn.push(tool);
      } else if (type === 'BuiltInRust') {
        result.rustBuiltIn.push(tool);
      }
    });

    return result;
  }, [allTools, toolTypeMap]);

  // Get tool type statistics
  const getToolStats = useCallback(() => {
    const stats: {
      external: number;
      webBuiltIn: number;
      rustBuiltin: number;
      total: number;
    } = {
      external: 0,
      webBuiltIn: 0,
      rustBuiltin: 0,
      total: toolTypeMap.size,
    };

    toolTypeMap.forEach((type) => {
      if (type === 'ExternalMCP') stats.external++;
      else if (type === 'BuiltInWeb') stats.webBuiltIn++;
      else if (type === 'BuiltInRust') stats.rustBuiltin++;
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

    // Lookup map (for debugging/inspection)
    toolTypeMap,

    // Status
    isWebReady,
    toolCounts: getToolStats(),
  };
};
