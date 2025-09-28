import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import type { MCPTool, MCPResponse } from '@/lib/mcp-types';
import { useEffect } from 'react';
import { useAsyncFn } from 'react-use';
import { useBuiltInTool } from '.';

const logger = getLogger('RustMCPToolProvider');

/**
 * RustMCPToolProvider registers a BuiltInService that exposes tools provided
 * by the Rust backend (tauri). It registers on mount and unregisters on
 * unmount. The service will list tools and delegate execution to the
 * rust backend hooks.
 */
export function RustMCPToolProvider() {
  const { register, unregister } = useBuiltInTool();
  const {
    listBuiltinServers,
    listBuiltinTools,
    callBuiltinTool,
    getServiceContext,
  } = useRustBackend();

  const [{ loading, value, error }, loadBuiltInServers] =
    useAsyncFn(async () => {
      const servers = await listBuiltinServers();

      const toolsByServer = await Promise.all(
        servers.map(async (s) => ({
          server: s,
          tools: await listBuiltinTools(s),
        })),
      );

      const serverTools: Record<string, MCPTool[]> = {};
      for (const entry of toolsByServer) {
        serverTools[entry.server] = entry.tools;
      }
      return serverTools;
    }, [listBuiltinServers, listBuiltinTools]);

  useEffect(() => {
    if (!loading && value) {
      Object.entries(value).forEach(([serviceId, tools]) => {
        const cachedTools = tools;

        register(serviceId, {
          listTools: () => cachedTools,
          loadService: async () => {
            // no-op: preloaded
          },
          unloadService: async () => {
            // no-op
          },
          executeTool: async (toolCall) => {
            const toolName = toolCall.function.name;

            // safely parse args
            let args: Record<string, unknown> = {};
            try {
              const raw = toolCall.function.arguments;
              if (typeof raw === 'string') {
                args = raw.length
                  ? (JSON.parse(raw) as Record<string, unknown>)
                  : {};
              } else if (typeof raw === 'object' && raw !== null) {
                args = raw as Record<string, unknown>;
              }
            } catch (e) {
              logger.warn('Failed parsing tool arguments; sending raw', {
                serviceId,
                toolName,
                error: e,
              });
              args = { raw: toolCall.function.arguments } as Record<
                string,
                unknown
              >;
            }

            const rawResult: MCPResponse<unknown> = await callBuiltinTool(
              serviceId,
              toolName,
              args,
            );
            return rawResult; // Rust backend already returns proper MCPResponse
          },
          getServiceContext: async () => {
            return await getServiceContext(serviceId);
          },
        });
      });

      return () => {
        Object.keys(value).forEach((s) => unregister(s));
      };
    }
    return undefined;
  }, [loading, value, register, unregister, callBuiltinTool]);

  // Log loader errors for visibility
  useEffect(() => {
    if (error) {
      logger.error('Failed to load built-in servers/tools', { error });
    }
  }, [error]);

  useEffect(() => {
    loadBuiltInServers();
  }, []);

  return null;
}
