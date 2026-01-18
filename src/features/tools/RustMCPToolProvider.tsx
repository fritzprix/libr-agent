import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import type { MCPTool, MCPResponse } from '@/lib/mcp-types';
import { useEffect } from 'react';
import { useAsyncFn } from 'react-use';
import {
  useBuiltInTool,
  ServiceContextOptions,
  ServiceContext,
  ServiceMetadata,
} from '.';

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
    listAvailableBuiltinServerDefinitions,
    listBuiltinTools,
    callBuiltinTool,
    getServiceContext,
  } = useRustBackend();

  const [{ loading, value, error }, loadBuiltInServers] =
    useAsyncFn(async () => {
      logger.debug(
        'Loading ALL available built-in server definitions from Rust backend',
      );

      // Use new API to get ALL available builtin server definitions (not just registered instances)
      const serverInfos = await listAvailableBuiltinServerDefinitions();
      logger.info('RustMCP: Fetched server definitions', {
        count: serverInfos.length,
        names: serverInfos.map((s) => s.name),
      });

      // Load tools for each server
      const serverData = await Promise.all(
        serverInfos.map(async (info) => {
          const tools = await listBuiltinTools(info.name);
          if (info.name === 'planning' || info.name === 'builtin_planning') {
            logger.info('RustMCP: Loaded planning tools', {
              serverName: info.name,
              toolCount: tools.length,
              toolNames: tools.map((t) => t.name),
            });
          }
          return {
            name: info.name,
            metadata: info.metadata,
            tools: tools,
          };
        }),
      );

      const serverMap: Record<
        string,
        { tools: MCPTool[]; metadata: ServiceMetadata }
      > = {};
      for (const data of serverData) {
        serverMap[data.name] = {
          tools: data.tools,
          metadata: data.metadata,
        };
      }

      logger.info('All available built-in server definitions loaded', {
        serverCount: serverInfos.length,
        servers: serverInfos.map((s) => ({
          name: s.name,
          displayName: s.metadata.displayName,
        })),
      });

      return serverMap;
    }, [listAvailableBuiltinServerDefinitions, listBuiltinTools]);

  useEffect(() => {
    if (!loading && value) {
      Object.entries(value).forEach(([serviceId, { tools, metadata }]) => {
        const cachedTools = tools;

        register(serviceId, {
          metadata, // Use runtime metadata from Rust backend
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
          getServiceContext: async (
            options?: ServiceContextOptions,
          ): Promise<ServiceContext<unknown>> => {
            const context = await getServiceContext(serviceId, options);
            return context;
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
