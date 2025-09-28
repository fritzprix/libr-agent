import { useMCPServer } from './use-mcp-server';
// import { useWebMCPTools } from './use-web-mcp';
import { useBuiltInTool } from '@/features/tools';
import { useCallback, useMemo, useRef, useEffect } from 'react';
import { MCPContent, MCPResponse, MCPTool, ServiceInfo } from '@/lib/mcp-types';
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

  // Cache for tool name resolution to improve performance
  const resolutionCacheRef = useRef<Map<string, string | null>>(new Map());

  // Stable references for external dependencies to prevent unnecessary re-renders
  const executeExternalRef = useRef(executeExternalMCP);
  const executeRustBuiltinRef = useRef(executeRustBuiltinTool);

  // Update refs when dependencies change
  useEffect(() => {
    executeExternalRef.current = executeExternalMCP;
    executeRustBuiltinRef.current = executeRustBuiltinTool;
  });

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

    // Clear cache when tool map changes to ensure consistency
    resolutionCacheRef.current.clear();

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

  // Intelligent tool name resolution with caching - maps LLM-called names to actual prefixed names
  const resolveToolName = useCallback(
    (calledToolName: string): string | null => {
      // Check cache first for improved performance
      const cached = resolutionCacheRef.current.get(calledToolName);
      if (cached !== undefined) {
        return cached;
      }

      let resolved: string | null = null;

      // 1. Check exact match first (already prefixed)
      if (toolTypeMap.has(calledToolName)) {
        resolved = calledToolName;
      } else {
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
                resolved = registeredName;
                break;
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
                resolved = registeredName;
                break;
              }
            }
          }
        }
      }

      if (!resolved) {
        // No resolution found
        logger.warn('Could not resolve tool name', {
          calledName: calledToolName,
          availableTools: Array.from(toolTypeMap.keys()),
        });
      }

      // Cache the result (including null results to avoid repeated failed lookups)
      resolutionCacheRef.current.set(calledToolName, resolved);
      return resolved;
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

  // Service info extraction utility function
  const extractServiceInfo = useCallback(
    (resolvedToolName: string, toolType: BackendType): ServiceInfo => {
      if (toolType === 'ExternalMCP') {
        const [serverName, ...toolNameParts] = resolvedToolName.split('__');
        return {
          serverName,
          toolName: toolNameParts.join('__'),
          backendType: toolType,
        };
      } else {
        const withoutPrefix = resolvedToolName.replace('builtin.', '');
        const [serverName, ...toolNameParts] = withoutPrefix.split('__');
        return {
          serverName,
          toolName: toolNameParts.join('__'),
          backendType: toolType,
        };
      }
    },
    [],
  );

  // Unified tool execution with stable references
  const executeToolCall = useCallback(
    async (toolCall: ToolCall): Promise<MCPResponse<unknown>> => {
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
        let response: MCPResponse<unknown>;

        if (toolType === 'BuiltInWeb') {
          throw new Error('Web MCP tools are currently disabled');
        } else if (toolType === 'BuiltInRust') {
          response = await executeRustBuiltinRef.current(actualToolCall);
        } else {
          response = await executeExternalRef.current(actualToolCall);
        }

        // Extract service info
        const serviceInfo = extractServiceInfo(resolvedToolName, toolType);

        // Add serviceInfo to MCPResponse content
        if (response.result?.content) {
          response.result.content = response.result.content.map(
            (content: MCPContent) => ({
              ...content,
              serviceInfo,
            }),
          );
        }

        return response;
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
    [], // Empty dependency array for stable reference
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
