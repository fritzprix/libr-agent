import {
  createMCPStructuredResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { getLogger } from '@/lib/logger';
import { mcpManagerTools } from './tools';
import { mcpServersCRUD, assistantsCRUD, createPage } from '@/lib/db/crud';
import { LocalDatabase } from '@/lib/db/service';
import type { MCPServerEntity } from '@/models/chat';

const logger = getLogger('MCPManagerServer');

// Response types for structured content
interface ListServersOutput {
  items: MCPServerEntity[];
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  hasNextPage: boolean;
  hasPreviousPage: boolean;
}

interface SearchServersOutput extends ListServersOutput {
  query: string;
}

interface CreateServerOutput {
  server: MCPServerEntity;
  message: string;
}

interface ConnectServerOutput {
  success: boolean;
  server: MCPServerEntity;
  scope: 'assistant' | 'global';
  message: string;
  assistantId?: string;
}

class MCPManagerServer implements WebMCPServer {
  name = 'mcp_manager';
  version = '1.0.0';
  description =
    'MCP server management tools for dynamic server registration and connection';
  tools = mcpManagerTools;

  // Context state (set by WebMCPContextSetter)
  private assistantId: string | null = null;
  private sessionId: string | null = null;

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    try {
      switch (name) {
        case 'list_servers':
          return await this.listServers(a);
        case 'search_server':
          return await this.searchServer(a);
        case 'create_server':
          return await this.createServer(a);
        case 'connect_server':
          return await this.connectServer(a);
        case 'disconnect_server':
          return await this.disconnectServer(a);
        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (error) {
      logger.error('Tool execution error', { name, error });
      const message = error instanceof Error ? error.message : String(error);
      return createMCPTextResponse(`Error: ${message}`);
    }
  }

  private async listServers(
    args: Record<string, unknown>,
  ): Promise<MCPResponse<ListServersOutput>> {
    const page = Number(args.page || 1);
    const pageSize = Number(args.pageSize || 20);
    const filterByAssistant = Boolean(args.filterByAssistant);
    const includeInactive = Boolean(args.includeInactive ?? true);

    let servers: MCPServerEntity[];

    if (filterByAssistant && this.assistantId) {
      // Get assistant's connected servers
      const assistant = await assistantsCRUD.read(this.assistantId);
      if (
        !assistant ||
        !assistant.mcpServerIds ||
        assistant.mcpServerIds.length === 0
      ) {
        const emptyPage = createPage<MCPServerEntity>([], page, pageSize, 0);
        return createMCPStructuredResponse(
          `No MCP servers connected to assistant "${assistant?.name || 'current'}"`,
          emptyPage,
        );
      }

      // Fetch servers by IDs
      const db = LocalDatabase.getInstance();
      servers = await db.mcpServers
        .where('id')
        .anyOf(assistant.mcpServerIds)
        .toArray();
    } else {
      // Get all servers
      const db = LocalDatabase.getInstance();
      servers = await db.mcpServers.toArray();
    }

    // Filter by active status
    if (!includeInactive) {
      servers = servers.filter((s: MCPServerEntity) => s.isActive);
    }

    // Sort by name
    servers.sort((a, b) => a.name.localeCompare(b.name));

    // Apply pagination
    const totalItems = servers.length;
    const result = createPage(servers, page, pageSize, totalItems);

    const summary = [
      `📋 MCP Servers (${result.totalItems} total)`,
      filterByAssistant ? `   Filtered by current assistant` : '',
      `   Page ${result.page}/${result.totalPages}`,
      `   Showing ${result.items.length} server(s)`,
    ]
      .filter(Boolean)
      .join('\n');

    return createMCPStructuredResponse(summary, result);
  }

  private async searchServer(
    args: Record<string, unknown>,
  ): Promise<MCPResponse<SearchServersOutput>> {
    const query = String(args.query || '')
      .toLowerCase()
      .trim();
    const page = Number(args.page || 1);
    const pageSize = Number(args.pageSize || 20);
    const byNameOnly = Boolean(args.byNameOnly ?? true);
    const includeInactive = Boolean(args.includeInactive ?? true);

    if (!query) {
      return createMCPTextResponse('Search query is required');
    }

    const db = LocalDatabase.getInstance();
    let servers = await db.mcpServers.toArray();

    // Filter by active status
    if (!includeInactive) {
      servers = servers.filter((s: MCPServerEntity) => s.isActive);
    }

    // Search filter
    servers = servers.filter((server: MCPServerEntity) => {
      const nameMatch = server.name.toLowerCase().includes(query);
      if (byNameOnly) return nameMatch;

      const descMatch = server.metadata?.description
        ?.toLowerCase()
        .includes(query);

      return nameMatch || descMatch;
    });

    // Sort by relevance (exact matches first, then starts-with, then contains)
    servers.sort((a: MCPServerEntity, b: MCPServerEntity) => {
      const aName = a.name.toLowerCase();
      const bName = b.name.toLowerCase();

      if (aName === query && bName !== query) return -1;
      if (aName !== query && bName === query) return 1;
      if (aName.startsWith(query) && !bName.startsWith(query)) return -1;
      if (!aName.startsWith(query) && bName.startsWith(query)) return 1;

      return aName.localeCompare(bName);
    });

    const totalItems = servers.length;
    const result = createPage(servers, page, pageSize, totalItems);

    const summary = [
      `🔍 Search Results for "${query}"`,
      `   Found ${result.totalItems} matching server(s)`,
      `   Page ${result.page}/${result.totalPages}`,
      `   Showing ${result.items.length} server(s)`,
    ].join('\n');

    return createMCPStructuredResponse(summary, {
      ...result,
      query,
    });
  }

  private async createServer(
    args: Record<string, unknown>,
  ): Promise<MCPResponse<CreateServerOutput>> {
    const name = String(args.name || '').trim();
    const description = String(args.description || '');
    const transport = args.transport as Record<string, unknown>;
    // Note: tags are accepted in args but not stored in metadata (not part of ServerMetadata interface)

    // Validate required fields
    if (!name) {
      return createMCPTextResponse('Server name is required');
    }

    if (!transport || !transport.type) {
      return createMCPTextResponse('Transport configuration is required');
    }

    const transportType = String(transport.type);
    if (transportType !== 'stdio' && transportType !== 'http') {
      return createMCPTextResponse('Transport type must be "stdio" or "http"');
    }

    // Validate transport-specific fields
    if (transportType === 'stdio' && !transport.command) {
      return createMCPTextResponse('Command is required for stdio transport');
    }
    if (transportType === 'http' && !transport.url) {
      return createMCPTextResponse('URL is required for http transport');
    }

    // Build server entity
    const now = new Date();
    const server: MCPServerEntity = {
      id: `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`,
      name,
      isActive: true,
      createdAt: now,
      updatedAt: now,
      transport: transport as MCPServerEntity['transport'],
      metadata: {
        description,
      },
    };

    // Try to save (will throw if name exists)
    try {
      await mcpServersCRUD.upsert(server);

      const summary = [
        `✅ MCP Server Created Successfully`,
        `   Name: ${server.name}`,
        `   ID: ${server.id}`,
        `   Transport: ${transportType}`,
        `   Status: Active`,
        ``,
        `💡 Use "connect_server" to enable this server for the current assistant.`,
      ].join('\n');

      return createMCPStructuredResponse(summary, {
        server,
        message: 'Server created successfully',
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);

      if (message.includes('name already exists')) {
        return createMCPTextResponse(
          `❌ Server name "${name}" already exists. Please choose a different name.`,
        );
      }

      throw error;
    }
  }

  private async connectServer(
    args: Record<string, unknown>,
  ): Promise<MCPResponse<ConnectServerOutput>> {
    const serverId = String(args.serverId || '');
    const serverName = String(args.serverName || '');
    const scope = String(args.scope || 'assistant');
    const autoStart = Boolean(args.autoStart ?? true);

    // Validate scope
    if (scope !== 'assistant' && scope !== 'global') {
      return createMCPTextResponse('Scope must be "assistant" or "global"');
    }

    // Find server
    let server: MCPServerEntity | undefined;

    if (serverId) {
      server = await mcpServersCRUD.read(serverId);
    } else if (serverName) {
      const db = LocalDatabase.getInstance();
      server = await db.mcpServers
        .filter(
          (s: MCPServerEntity) =>
            s.name.toLowerCase() === serverName.toLowerCase(),
        )
        .first();
    } else {
      return createMCPTextResponse('Either serverId or serverName is required');
    }

    if (!server) {
      return createMCPTextResponse(
        `Server not found: ${serverId || serverName}`,
      );
    }

    // Connect based on scope
    if (scope === 'assistant') {
      if (!this.assistantId) {
        return createMCPTextResponse(
          '❌ Assistant context not available. Cannot connect to assistant scope.',
        );
      }

      const assistant = await assistantsCRUD.read(this.assistantId);
      if (!assistant) {
        return createMCPTextResponse(
          `Assistant not found: ${this.assistantId}`,
        );
      }

      // Add to assistant's server list if not already connected
      const serverIds = assistant.mcpServerIds || [];
      if (!serverIds.includes(server.id)) {
        assistant.mcpServerIds = [...serverIds, server.id];
        await assistantsCRUD.upsert(assistant);
      }

      const summary = [
        `✅ Server Connected to Assistant`,
        `   Server: ${server.name}`,
        `   Assistant: ${assistant.name}`,
        `   Scope: assistant`,
        autoStart
          ? `   Status: Starting...`
          : `   Status: Registered (manual start required)`,
      ].join('\n');

      return createMCPStructuredResponse(summary, {
        success: true,
        server,
        scope: 'assistant',
        assistantId: this.assistantId,
        message: `Server "${server.name}" connected to assistant "${assistant.name}"`,
      });
    } else {
      // Global scope: mark server as globally enabled
      server.isActive = true;
      server.updatedAt = new Date();
      await mcpServersCRUD.upsert(server);

      const summary = [
        `✅ Server Enabled Globally`,
        `   Server: ${server.name}`,
        `   Scope: global`,
        `   Status: Active`,
      ].join('\n');

      return createMCPStructuredResponse(summary, {
        success: true,
        server,
        scope: 'global',
        message: `Server "${server.name}" enabled globally`,
      });
    }
  }

  private async disconnectServer(
    args: Record<string, unknown>,
  ): Promise<MCPResponse<unknown>> {
    const serverId = String(args.serverId || '');
    const serverName = String(args.serverName || '');
    const scope = String(args.scope || 'assistant');

    // Find server
    let server: MCPServerEntity | undefined;

    if (serverId) {
      server = await mcpServersCRUD.read(serverId);
    } else if (serverName) {
      const db = LocalDatabase.getInstance();
      server = await db.mcpServers
        .filter(
          (s: MCPServerEntity) =>
            s.name.toLowerCase() === serverName.toLowerCase(),
        )
        .first();
    } else {
      return createMCPTextResponse('Either serverId or serverName is required');
    }

    if (!server) {
      return createMCPTextResponse(
        `Server not found: ${serverId || serverName}`,
      );
    }

    if (scope === 'assistant') {
      if (!this.assistantId) {
        return createMCPTextResponse('❌ Assistant context not available.');
      }

      const assistant = await assistantsCRUD.read(this.assistantId);
      if (!assistant) {
        return createMCPTextResponse(
          `Assistant not found: ${this.assistantId}`,
        );
      }

      const serverIds = assistant.mcpServerIds || [];
      assistant.mcpServerIds = serverIds.filter((id) => id !== server!.id);
      await assistantsCRUD.upsert(assistant);

      return createMCPStructuredResponse(
        `✅ Server "${server.name}" disconnected from assistant "${assistant.name}"`,
        { success: true, server, scope: 'assistant' },
      );
    } else {
      server.isActive = false;
      server.updatedAt = new Date();
      await mcpServersCRUD.upsert(server);

      return createMCPStructuredResponse(
        `✅ Server "${server.name}" disabled globally`,
        { success: true, server, scope: 'global' },
      );
    }
  }

  async switchContext(options: ServiceContextOptions): Promise<void> {
    this.assistantId = options.assistantId || null;
    this.sessionId = options.sessionId || null;
    logger.debug('Context switched', {
      assistantId: this.assistantId,
      sessionId: this.sessionId,
    });
  }

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<unknown>> {
    const db = LocalDatabase.getInstance();
    const totalServers = await db.mcpServers.count();
    const activeServers = await db.mcpServers
      .filter((s: MCPServerEntity) => s.isActive)
      .count();

    // Use options.assistantId if provided, otherwise use the current context
    const assistantId = options?.assistantId || this.assistantId;

    let assistantInfo = '';
    if (assistantId) {
      const assistant = await assistantsCRUD.read(assistantId);
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
        assistantId,
        sessionId: options?.sessionId || this.sessionId,
      },
    };
  }
}

export default new MCPManagerServer();
