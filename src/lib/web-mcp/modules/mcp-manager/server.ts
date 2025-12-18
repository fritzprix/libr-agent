import { createMCPErrorToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { mcpManagerTools } from './tools';
import { getServices } from './services/service-provider';
import { listServers } from './handlers/list';
import { searchServer } from './handlers/search';
import { createServer } from './handlers/create';
import { connectServer, disconnectServer } from './handlers/connect';

// Context state (module-scoped, set by WebMCPContextSetter)
let assistantId: string | null = null;
let sessionId: string | null = null;

const mcpManagerServer: WebMCPServer = {
  name: 'mcp_manager',
  displayName: 'MCP Server Manager',
  description: 'Create, search, and manage MCP server configurations',
  version: '1.0.0',
  tools: mcpManagerTools,

  async callTool(name: string, args: unknown): Promise<MCPResult<unknown>> {
    const a = (args || {}) as Record<string, unknown>;
    const { mcpService, assistantService } = await getServices();

    try {
      switch (name) {
        case 'listServers':
          return await listServers(
            mcpService,
            assistantService,
            a,
            assistantId,
          );
        case 'searchServer':
          return await searchServer(mcpService, a);
        case 'createServer':
          return await createServer(mcpService, a);
        case 'connectServer':
          return await connectServer(
            mcpService,
            assistantService,
            a,
            assistantId,
          );
        case 'disconnectServer':
          return await disconnectServer(
            mcpService,
            assistantService,
            a,
            assistantId,
          );
        default:
          return createMCPErrorToolResult(`Unknown tool: ${name}`, {
            toolName: name,
            availableTools: mcpManagerTools.map((t) => t.name),
          });
      }
    } catch (error) {
      return createMCPErrorToolResult(
        `Error executing tool ${name}: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  },

  async switchContext(options: ServiceContextOptions): Promise<void> {
    assistantId = options.assistantId || null;
    sessionId = options.sessionId || null;
  },

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<unknown>> {
    const { mcpService, assistantService } = await getServices();
    const totalServers = await mcpService.count();
    const allServers = await mcpService.getAll();
    const activeServers = allServers.filter((s) => s.isActive).length;

    // Use options.assistantId if provided, otherwise use the current context
    const contextAssistantId = options?.assistantId || assistantId;

    let assistantInfo = '';
    if (contextAssistantId) {
      const assistant = await assistantService.getById(contextAssistantId);
      const connectedCount = assistant?.mcpServerIds?.length || 0;
      assistantInfo = `\n**Current Assistant**: ${assistant?.name || 'Unknown'}\n**Connected Servers**: ${connectedCount}`;
    }

    const contextPrompt = [
      `# MCP Manager Server Status`,
      `**Server**: mcp_manager`,
      `**Status**: Active`,
      `**Total Servers**: ${totalServers}`,
      `**Active Servers**: ${activeServers}`,
      assistantInfo,
    ]
      .filter(Boolean)
      .join('\n');

    return {
      contextPrompt,
      structuredState: {
        totalServers,
        activeServers,
        assistantId: contextAssistantId,
        sessionId: options?.sessionId || sessionId,
      },
    };
  },
};

export default mcpManagerServer;
