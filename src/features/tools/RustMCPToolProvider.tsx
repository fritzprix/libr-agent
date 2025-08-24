import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import type { MCPTool, MCPResponse } from '@/lib/mcp-types';
import { normalizeToolResult } from '@/lib/mcp-types';
import { useEffect } from 'react';
import { useBuiltInTool, type BuiltInService } from '.';
import type { ToolCall } from '@/models/chat';

const logger = getLogger('RustMCPToolProvider');

/**
 * RustMCPToolProvider registers a BuiltInService that exposes tools provided
 * by the Rust backend (tauri). It registers on mount and unregisters on
 * unmount. The service will list tools and delegate execution to the
 * rust backend hooks.
 */
export function RustMCPToolProvider() {
  const { register, unregister } = useBuiltInTool();
  const { listBuiltinServers, listBuiltinTools, callBuiltinTool } =
    useRustBackend();

  useEffect(() => {
    let mounted = true;

    const serviceId = 'rust';

    const service = {
      listTools: () => {
        // We'll lazily fetch tools when loadService runs. For now return empty array.
        return [] as MCPTool[];
      },
  executeTool: async (toolCall: ToolCall): Promise<MCPResponse> => {
        // toolCall.function.name is the tool name (without builtin prefix)
        const toolName = toolCall.function.name;
        // call into rust backend
        logger.debug('Rust service executing tool', { toolName, toolCall });
        const argsRaw = toolCall.function.arguments;
        let args: Record<string, unknown> = {};
        if (typeof argsRaw === 'string') {
          if (argsRaw.length === 0) {
            args = {};
          } else {
            try {
              const parsed = JSON.parse(argsRaw);
              args = (typeof parsed === 'object' && parsed !== null) ? (parsed as Record<string, unknown>) : { value: parsed };
            } catch (e) {
              logger.warn('Failed to parse args for rust tool call, wrapping raw string', { toolName, argsRaw, e });
              args = { raw: argsRaw };
            }
          }
        } else if (typeof argsRaw === 'object' && argsRaw !== null) {
          args = argsRaw as Record<string, unknown>;
        } else {
          args = {};
        }

        // Delegate to rust backend
        const rawResult = await callBuiltinTool(serviceId, toolName, args);
        // normalize to MCPResponse expected by BuiltInService
        return normalizeToolResult(rawResult, toolName);
      },
      loadService: async () => {
        // No-op or could prefetch tools
        try {
          const servers = await listBuiltinServers();
          logger.debug('Rust builtin servers', { servers });
          // Optionally fetch tools for the first server
          if (servers && servers.length > 0) {
            const tools = await listBuiltinTools();
            // When tools are needed, the BuiltInToolProvider will call listTools on this service.
            logger.debug('Fetched rust tools for service', { serviceId, toolCount: tools?.length });
          }
        } catch (err) {
          logger.error('Failed to load rust builtin tools', err);
        }
      },
      unloadService: async () => {
        // No special cleanup required
      },
    } as const;

  register(serviceId, service as BuiltInService);

    return () => {
      if (!mounted) return;
      unregister(serviceId);
      mounted = false;
    };
  }, [register, unregister, listBuiltinServers, listBuiltinTools, callBuiltinTool]);

  return null;
}
