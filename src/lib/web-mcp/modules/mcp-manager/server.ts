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
import {
  createBM25Index,
  defaultTokenizer,
  clearBM25Cache,
} from '@/lib/search/bm25';

const logger = getLogger('MCPManagerServer');

// Input type interfaces for type safety
interface ListServersInput {
  page?: number;
  pageSize?: number;
  filterByAssistant?: boolean;
  includeInactive?: boolean;
}

interface SearchServersInput {
  query: string;
  page?: number;
  pageSize?: number;
  searchMode?: 'bm25' | 'simple';
  byNameOnly?: boolean;
  includeInactive?: boolean;
  weights?: {
    nameWeight?: number;
    descWeight?: number;
  };
}

interface CreateServerInput {
  name: string;
  description?: string;
  transport: unknown;
  tags?: string[];
}

interface ConnectInput {
  serverId?: string;
  serverName?: string;
  scope?: 'assistant' | 'global';
  autoStart?: boolean;
}

interface DisconnectInput {
  serverId?: string;
  serverName?: string;
  scope?: 'assistant' | 'global';
}

// Transport type guards
interface StdioTransport {
  type: 'stdio';
  command: string;
  args?: string[];
  env?: Record<string, string>;
}

interface HttpTransport {
  type: 'http';
  url: string;
  headers?: Record<string, string>;
}

function isStdioTransport(t: unknown): t is StdioTransport {
  return (
    typeof t === 'object' &&
    t !== null &&
    'type' in t &&
    t.type === 'stdio' &&
    'command' in t &&
    typeof t.command === 'string'
  );
}

function isHttpTransport(t: unknown): t is HttpTransport {
  return (
    typeof t === 'object' &&
    t !== null &&
    'type' in t &&
    t.type === 'http' &&
    'url' in t &&
    typeof t.url === 'string'
  );
}

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
  mode?: string;
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

// Context state (module-scoped, set by WebMCPContextSetter)
let assistantId: string | null = null;
let sessionId: string | null = null;

/**
 * Helper function to find a server by ID or name
 */
async function findServer(
  serverId?: string,
  serverName?: string,
): Promise<MCPServerEntity | undefined> {
  if (serverId) {
    return await mcpServersCRUD.read(serverId);
  }
  if (serverName) {
    const db = LocalDatabase.getInstance();
    return await db.mcpServers
      .filter((s) => s.name.toLowerCase() === serverName.toLowerCase())
      .first();
  }
  return undefined;
}

/**
 * Normalize pagination including pageSize=-1 for all results
 */
function normalizePagination(
  items: MCPServerEntity[],
  page: number,
  pageSize: number,
): ListServersOutput {
  if (pageSize === -1) {
    // Return all items on page 1
    return {
      items,
      page: 1,
      pageSize: items.length,
      totalPages: 1,
      totalItems: items.length,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  // Use standard pagination helper
  return createPage(items, page, pageSize, items.length);
}

async function listServers(
  args: Record<string, unknown>,
): Promise<MCPResponse<ListServersOutput>> {
  const input: ListServersInput = {
    page: args.page !== undefined ? Number(args.page) : 1,
    pageSize: args.pageSize !== undefined ? Number(args.pageSize) : 20,
    filterByAssistant: Boolean(args.filterByAssistant),
    includeInactive: Boolean(args.includeInactive ?? true),
  };

  let servers: MCPServerEntity[];

  if (input.filterByAssistant && assistantId) {
    // Get assistant's connected servers
    const assistant = await assistantsCRUD.read(assistantId);
    if (
      !assistant ||
      !assistant.mcpServerIds ||
      assistant.mcpServerIds.length === 0
    ) {
      const emptyPage = normalizePagination([], input.page!, input.pageSize!);
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
  if (!input.includeInactive) {
    servers = servers.filter((s: MCPServerEntity) => s.isActive);
  }

  // Sort by name
  servers.sort((a, b) => a.name.localeCompare(b.name));

  // Apply pagination
  const result = normalizePagination(servers, input.page!, input.pageSize!);

  // Build summary with server details
  const summaryLines = [
    `📋 MCP Servers (${result.totalItems} total)`,
    input.filterByAssistant ? `   Filtered by current assistant` : '',
    `   Page ${result.page}/${result.totalPages}`,
    `   Showing ${result.items.length} server(s)`,
  ];

  // Add server details
  if (result.items.length > 0) {
    summaryLines.push('');
    summaryLines.push('Servers:');
    result.items.forEach((server, idx) => {
      const status = server.isActive ? '🟢' : '🔴';
      const desc = server.metadata?.description
        ? ` - ${server.metadata.description.slice(0, 60)}${server.metadata.description.length > 60 ? '...' : ''}`
        : '';
      summaryLines.push(`  ${idx + 1}. ${status} ${server.name}${desc}`);
    });
  }

  const summary = summaryLines.filter(Boolean).join('\n');

  return createMCPStructuredResponse(summary, result);
}

async function searchServer(
  args: Record<string, unknown>,
): Promise<MCPResponse<SearchServersOutput>> {
  const input: SearchServersInput = {
    query: String(args.query || '').trim(),
    page: args.page !== undefined ? Number(args.page) : 1,
    pageSize: args.pageSize !== undefined ? Number(args.pageSize) : 20,
    searchMode: (args.searchMode as 'bm25' | 'simple') || 'bm25',
    byNameOnly: Boolean(args.byNameOnly ?? true),
    includeInactive: Boolean(args.includeInactive ?? true),
    weights: args.weights as SearchServersInput['weights'],
  };

  if (!input.query) {
    return createMCPTextResponse('Search query is required');
  }

  const db = LocalDatabase.getInstance();
  let servers = await db.mcpServers.toArray();

  // Filter by active status
  if (!input.includeInactive) {
    servers = servers.filter((s: MCPServerEntity) => s.isActive);
  }

  // Apply search based on mode
  if (input.searchMode === 'simple') {
    // Simple substring matching (backward compatibility)
    const query = input.query.toLowerCase();

    servers = servers.filter((server: MCPServerEntity) => {
      const nameMatch = server.name.toLowerCase().includes(query);
      if (input.byNameOnly) return nameMatch;

      const descMatch = server.metadata?.description
        ?.toLowerCase()
        .includes(query);

      return nameMatch || descMatch;
    });

    // Improved relevance sorting: exact > startsWith > contains
    const scoreLevel = (name: string) => {
      const lowerName = name.toLowerCase();
      if (lowerName === query) return 3;
      if (lowerName.startsWith(query)) return 2;
      if (lowerName.includes(query)) return 1;
      return 0;
    };

    servers.sort((a: MCPServerEntity, b: MCPServerEntity) => {
      const scoreA = scoreLevel(a.name);
      const scoreB = scoreLevel(b.name);
      if (scoreA !== scoreB) return scoreB - scoreA;
      return a.name.localeCompare(b.name);
    });

    const result = normalizePagination(servers, input.page!, input.pageSize!);

    // Build summary with search results
    const summaryLines = [
      `🔍 Search Results for "${input.query}" (simple)`,
      `   Found ${result.totalItems} matching server(s)`,
      `   Page ${result.page}/${result.totalPages}`,
      `   Showing ${result.items.length} server(s)`,
    ];

    // Add top results
    if (result.items.length > 0) {
      summaryLines.push('');
      summaryLines.push('Top Results:');
      result.items.slice(0, 5).forEach((server, idx) => {
        const status = server.isActive ? '🟢' : '🔴';
        const matchType =
          server.name.toLowerCase() === query
            ? '[exact]'
            : server.name.toLowerCase().startsWith(query)
              ? '[starts]'
              : '[contains]';
        summaryLines.push(
          `  ${idx + 1}. ${status} ${server.name} ${matchType}`,
        );
      });
      if (result.items.length > 5) {
        summaryLines.push(`  ... and ${result.items.length - 5} more`);
      }
    }

    const summary = summaryLines.join('\n');

    return createMCPStructuredResponse(summary, {
      ...result,
      query: input.query,
      mode: 'simple',
    });
  }

  // BM25 mode (default)
  const nameWeight = input.weights?.nameWeight ?? 2.0;
  const descWeight = input.weights?.descWeight ?? 1.0;

  // Build BM25 documents with weighted token duplication
  const docs = servers.map((server) => {
    const nameTokens = defaultTokenizer(server.name);
    const descTokens = defaultTokenizer(server.metadata?.description || '');

    // Duplicate tokens based on weights (round to nearest integer, min 1)
    const weightedNameTokens = nameTokens.flatMap((token) =>
      Array(Math.max(1, Math.round(nameWeight))).fill(token),
    );
    const weightedDescTokens = descTokens.flatMap((token) =>
      Array(Math.max(1, Math.round(descWeight))).fill(token),
    );

    return {
      id: server.id,
      tokens: [...weightedNameTokens, ...weightedDescTokens],
    };
  });

  // Create or retrieve cached BM25 index
  const index = createBM25Index(docs);
  const queryTokens = defaultTokenizer(input.query);
  const scores = index.score(queryTokens);

  // Sort by BM25 score descending, tie-breaker by name alphabetically
  servers.sort((a, b) => {
    const scoreA = scores.get(a.id) || 0;
    const scoreB = scores.get(b.id) || 0;
    if (scoreA !== scoreB) return scoreB - scoreA;
    return a.name.localeCompare(b.name);
  });

  const result = normalizePagination(servers, input.page!, input.pageSize!);

  // Build summary with BM25 search results
  const summaryLines = [
    `🔎 BM25 Results for "${input.query}" (name×${nameWeight}, desc×${descWeight})`,
    `   Found ${result.totalItems} matching server(s)`,
    `   Page ${result.page}/${result.totalPages}`,
    `   Showing ${result.items.length} server(s)`,
  ];

  // Add top results with scores
  if (result.items.length > 0) {
    summaryLines.push('');
    summaryLines.push('Top Results (by relevance):');
    result.items.slice(0, 5).forEach((server, idx) => {
      const status = server.isActive ? '🟢' : '🔴';
      const score = scores.get(server.id) || 0;
      const scoreStr = score > 0 ? ` [score: ${score.toFixed(2)}]` : '';
      summaryLines.push(`  ${idx + 1}. ${status} ${server.name}${scoreStr}`);
    });
    if (result.items.length > 5) {
      summaryLines.push(`  ... and ${result.items.length - 5} more`);
    }
  }

  const summary = summaryLines.join('\n');

  return createMCPStructuredResponse(summary, {
    ...result,
    query: input.query,
    mode: 'bm25',
  });
}

async function createServer(
  args: Record<string, unknown>,
): Promise<MCPResponse<CreateServerOutput>> {
  const input: CreateServerInput = {
    name: String(args.name || '').trim(),
    description: args.description ? String(args.description) : undefined,
    transport: args.transport,
    tags: args.tags as string[] | undefined,
  };

  // Validate required fields
  if (!input.name) {
    return createMCPTextResponse('Server name is required');
  }

  // Validate transport with type guards
  if (
    !(isStdioTransport(input.transport) || isHttpTransport(input.transport))
  ) {
    return createMCPTextResponse(
      'Invalid transport configuration. Must be stdio (with command) or http (with url).',
    );
  }

  // Generate unique ID using crypto.randomUUID() with fallback
  const id =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? `mcp-${crypto.randomUUID()}`
      : `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  // Build server entity
  const now = new Date();
  const server: MCPServerEntity = {
    id,
    name: input.name,
    isActive: true,
    createdAt: now,
    updatedAt: now,
    transport: input.transport as MCPServerEntity['transport'],
    metadata: {
      description: input.description || '',
    },
  };

  // Try to save (will throw if name exists)
  try {
    await mcpServersCRUD.upsert(server);

    // Clear BM25 cache when data changes
    clearBM25Cache();

    const transportType = input.transport.type;
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
        `❌ Server name "${input.name}" already exists. Please choose a different name.`,
      );
    }

    throw error;
  }
}

async function connectServer(
  args: Record<string, unknown>,
): Promise<MCPResponse<ConnectServerOutput>> {
  const input: ConnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
    autoStart: Boolean(args.autoStart ?? true),
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPTextResponse('Scope must be "assistant" or "global"');
  }

  // Find server using helper
  const server = await findServer(input.serverId, input.serverName);

  if (!server) {
    return createMCPTextResponse(
      `Server not found: ${input.serverId || input.serverName}`,
    );
  }

  // Connect based on scope
  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPTextResponse(
        '❌ Assistant context not available. Cannot connect to assistant scope.',
      );
    }

    const assistant = await assistantsCRUD.read(assistantId);
    if (!assistant) {
      return createMCPTextResponse(`Assistant not found: ${assistantId}`);
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
      input.autoStart
        ? `   Status: Starting...`
        : `   Status: Registered (manual start required)`,
    ].join('\n');

    return createMCPStructuredResponse(summary, {
      success: true,
      server,
      scope: 'assistant',
      assistantId,
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

async function disconnectServer(
  args: Record<string, unknown>,
): Promise<MCPResponse<unknown>> {
  const input: DisconnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPTextResponse('Scope must be "assistant" or "global"');
  }

  // Find server using helper
  const server = await findServer(input.serverId, input.serverName);

  if (!server) {
    return createMCPTextResponse(
      `Server not found: ${input.serverId || input.serverName}`,
    );
  }

  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPTextResponse('❌ Assistant context not available.');
    }

    const assistant = await assistantsCRUD.read(assistantId);
    if (!assistant) {
      return createMCPTextResponse(`Assistant not found: ${assistantId}`);
    }

    const serverIds = assistant.mcpServerIds || [];
    assistant.mcpServerIds = serverIds.filter((id) => id !== server.id);
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

const mcpManagerServer: WebMCPServer = {
  name: 'mcp_manager',
  version: '1.0.0',
  description:
    'MCP server management tools for dynamic server registration and connection',
  tools: mcpManagerTools,

  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    try {
      switch (name) {
        case 'list_servers':
          return await listServers(a);
        case 'search_server':
          return await searchServer(a);
        case 'create_server':
          return await createServer(a);
        case 'connect_server':
          return await connectServer(a);
        case 'disconnect_server':
          return await disconnectServer(a);
        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (error) {
      logger.error('Tool execution error', { name, error });
      const message = error instanceof Error ? error.message : String(error);
      return createMCPTextResponse(`Error: ${message}`);
    }
  },

  async switchContext(options: ServiceContextOptions): Promise<void> {
    assistantId = options.assistantId || null;
    sessionId = options.sessionId || null;
    logger.debug('Context switched', {
      assistantId,
      sessionId,
    });
  },

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<unknown>> {
    const db = LocalDatabase.getInstance();
    const totalServers = await db.mcpServers.count();
    const activeServers = await db.mcpServers
      .filter((s: MCPServerEntity) => s.isActive)
      .count();

    // Use options.assistantId if provided, otherwise use the current context
    const contextAssistantId = options?.assistantId || assistantId;

    let assistantInfo = '';
    if (contextAssistantId) {
      const assistant = await assistantsCRUD.read(contextAssistantId);
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
