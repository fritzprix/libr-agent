import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { mcpManagerTools } from './tools';
import { createPage } from '@/lib/db/crud';
import { dbService } from '@/lib/db/service';
import type { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
import {
  createBM25Index,
  defaultTokenizer,
  clearBM25Cache,
} from '@/lib/search/bm25';
import {
  McpServerService,
  IMcpServerService,
} from '@/lib/services/mcp-server-service';
import {
  AssistantService,
  IAssistantService,
} from '@/lib/services/assistant-service';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';

let cachedServices: {
  mcpService: IMcpServerService;
  assistantService: IAssistantService;
  agentHubUrl: string | undefined;
} | null = null;

async function getServices() {
  const agentHubUrlObj = await dbService.objects.read('agentHubUrl');
  const agentHubUrl = agentHubUrlObj?.value as string;

  if (cachedServices && cachedServices.agentHubUrl === agentHubUrl) {
    return {
      mcpService: cachedServices.mcpService,
      assistantService: cachedServices.assistantService,
    };
  }

  const mcpService = new McpServerService(agentHubUrl);
  const assistantService = new AssistantService(agentHubUrl);

  cachedServices = {
    mcpService,
    assistantService,
    agentHubUrl,
  };

  return { mcpService, assistantService };
}

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

/**
 * Validates environment variables or headers
 * Throws error if value is a string (common AI mistake) to force correction
 */
function validateRecordField(
  fieldName: string,
  value: unknown,
): Record<string, string> | undefined {
  if (!value) return undefined;

  // Check for string input (common AI mistake)
  if (typeof value === 'string') {
    throw new Error(
      `[Format Error] Invalid type for parameter '${fieldName}'.\n` +
      `Expected: Object (Record<string, string>)\n` +
      `Received: String (JSON string)\n\n` +
      `❌ Common Mistake: You provided a JSON string instead of a raw JSON object.\n` +
      `✅ Correct Usage:\n` +
      `   "${fieldName}": { "KEY": "VALUE" }\n\n` +
      `⛔ Incorrect Usage:\n` +
      `   "${fieldName}": "{\\"KEY\\": \\"VALUE\\"}"`,
    );
  }

  // Check for non-object input
  if (typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(
      `[Format Error] Invalid type for parameter '${fieldName}'.\n` +
      `Expected: Object\n` +
      `Received: ${Array.isArray(value) ? 'Array' : typeof value}\n` +
      `Please provide a key-value object.`,
    );
  }

  return value as Record<string, string>;
}

/**
 * Normalizes transport configuration to ensure proper types
 * Handles cases where AI agents provide JSON strings instead of objects
 */
function normalizeTransportConfig(transport: unknown): TransportConfig {
  if (!transport || typeof transport !== 'object') {
    throw new Error('Transport configuration is required');
  }

  const t = transport as Record<string, unknown>;

  if (t.type === 'stdio') {
    if (!t.command || typeof t.command !== 'string') {
      throw new Error('Stdio transport requires a command string');
    }

    // Validate args if present
    if (t.args !== undefined) {
      if (typeof t.args === 'string') {
        throw new Error(
          `[Format Error] Invalid type for parameter 'args'.\n` +
          `Expected: Array of strings (e.g. ["arg1", "arg2"])\n` +
          `Received: String\n` +
          `❌ Common Mistake: You provided a JSON string instead of a raw Array.\n` +
          `✅ Correct Usage: "args": ["--flag", "value"]`,
        );
      }
      if (!Array.isArray(t.args)) {
        throw new Error(`[Format Error] 'args' must be an array of strings.`);
      }
    }

    return {
      type: 'stdio',
      command: t.command,
      args: t.args ? (t.args as unknown[]).map(String) : undefined,
      env: validateRecordField('env', t.env),
    };
  }

  if (t.type === 'http') {
    if (!t.url || typeof t.url !== 'string') {
      throw new Error('HTTP transport requires a url string');
    }

    return {
      type: 'http',
      url: t.url,
      protocolVersion:
        typeof t.protocolVersion === 'string' ? t.protocolVersion : undefined,
      sessionId: typeof t.sessionId === 'string' ? t.sessionId : undefined,
      headers: validateRecordField('headers', t.headers),
      enableSSE: typeof t.enableSSE === 'boolean' ? t.enableSSE : undefined,
      security:
        t.security && typeof t.security === 'object'
          ? (t.security as {
            enableDnsRebindingProtection?: boolean;
            allowedOrigins?: string[];
            allowedHosts?: string[];
          })
          : undefined,
    } as Extract<TransportConfig, { type: 'http' }>;
  }

  throw new Error(`Unsupported transport type: ${String(t.type)}`);
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
  service: IMcpServerService,
  serverId?: string,
  serverName?: string,
): Promise<MCPServerEntity | undefined> {
  if (serverId) {
    return await service.getById(serverId);
  }
  if (serverName) {
    return await service.getByName(serverName);
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
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
): Promise<MCPResult<ListServersOutput>> {
  const input: ListServersInput = {
    page: args.page !== undefined ? Number(args.page) : 1,
    pageSize: args.pageSize !== undefined ? Number(args.pageSize) : 20,
    filterByAssistant: Boolean(args.filterByAssistant),
    includeInactive: Boolean(args.includeInactive ?? true),
  };

  let servers: MCPServerEntity[];

  if (input.filterByAssistant && assistantId) {
    // Get assistant's connected servers
    const assistant = await assistantService.getById(assistantId);
    if (
      !assistant ||
      !assistant.mcpServerIds ||
      assistant.mcpServerIds.length === 0
    ) {
      const emptyPage = normalizePagination([], input.page!, input.pageSize!);
      return createMCPStructuredToolResult(
        `No MCP servers connected to assistant "${assistant?.name || 'current'}"`,
        emptyPage,
      );
    }

    // Fetch servers by IDs
    // Note: IMcpServerService doesn't have getByIds, so we fetch all and filter or fetch individually
    // For efficiency with remote, we might want to add getByIds to service, but for now let's fetch all
    // or fetch individually if count is small.
    // Let's fetch all for now as it's safer for remote consistency if getByIds isn't available
    const allServers = await mcpService.getAll();
    servers = allServers.filter((s) => assistant.mcpServerIds?.includes(s.id));
  } else {
    // Get all servers
    servers = await mcpService.getAll();
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

  return createMCPStructuredToolResult(summary, result);
}

async function searchServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<SearchServersOutput>> {
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
    return createMCPErrorToolResult(
      'Search query is required',
    ) as MCPResult<SearchServersOutput>;
  }

  let servers = await mcpService.getAll();

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

    // Handle no results with improved guidance
    if (result.totalItems === 0) {
      const allServers = await mcpService.getAll();
      const totalCount = allServers.length;
      const activeCount = allServers.filter((s) => s.isActive).length;

      const suggestions: string[] = [];

      // Suggest switching to BM25 mode
      suggestions.push('Try searchMode: "bm25" for fuzzy matching');

      // Suggest browsing if database is small
      if (totalCount < 20) {
        suggestions.push('Use list_servers to browse all servers');
      } else {
        suggestions.push('Try different or shorter keywords');
      }

      // Suggest including inactive if filtered
      if (!input.includeInactive && totalCount > activeCount) {
        suggestions.push('Set includeInactive: true to search all servers');
      }

      return new MCPResponseBuilder({
        ...result,
        query: input.query,
        mode: 'simple',
        databaseStats: { total: totalCount, active: activeCount },
        suggestions,
      })
        .withMessage(
          `No servers found matching "${input.query}".\n` +
          `Database has ${activeCount} active servers (${totalCount} total).`,
        )
        .withSuggestions(suggestions)
        .asSuccess();
    }

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

    return createMCPStructuredToolResult(summary, {
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

  // Filter out servers with score of 0 (no match) and sort by BM25 score descending
  servers = servers.filter((server) => {
    const score = scores.get(server.id) || 0;
    return score > 0;
  });

  servers.sort((a, b) => {
    const scoreA = scores.get(a.id) || 0;
    const scoreB = scores.get(b.id) || 0;
    if (scoreA !== scoreB) return scoreB - scoreA;
    return a.name.localeCompare(b.name);
  });

  const result = normalizePagination(servers, input.page!, input.pageSize!);

  // Handle no results with improved guidance
  if (result.totalItems === 0) {
    const allServers = await mcpService.getAll();
    const totalCount = allServers.length;
    const activeCount = allServers.filter((s) => s.isActive).length;

    const suggestions: string[] = [];

    // Suggest switching to simple mode
    suggestions.push('Try searchMode: "simple" for exact matching');

    // Suggest browsing if database is small
    if (totalCount < 20) {
      suggestions.push('Use list_servers to browse all servers');
    } else {
      suggestions.push('Try broader or alternative keywords');
    }

    // Suggest including inactive if filtered
    if (!input.includeInactive && totalCount > activeCount) {
      suggestions.push('Set includeInactive: true to search all servers');
    }

    return new MCPResponseBuilder({
      ...result,
      query: input.query,
      mode: 'bm25',
      weights: { name: nameWeight, desc: descWeight },
      databaseStats: { total: totalCount, active: activeCount },
      suggestions,
    })
      .withMessage(
        `No servers found matching "${input.query}" (BM25 search).\n` +
        `Database has ${activeCount} active servers (${totalCount} total).`,
      )
      .withSuggestions(suggestions)
      .asSuccess();
  }

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

  return createMCPStructuredToolResult(summary, {
    ...result,
    query: input.query,
    mode: 'bm25',
  });
}

async function createServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<CreateServerOutput>> {
  const input: CreateServerInput = {
    name: String(args.name || '').trim(),
    description: args.description ? String(args.description) : undefined,
    transport: args.transport,
    tags: args.tags as string[] | undefined,
  };

  // Validate required fields
  if (!input.name) {
    return createMCPErrorToolResult(
      'Server name is required',
    ) as MCPResult<CreateServerOutput>;
  }

  // Normalize and validate transport configuration
  let normalizedTransport: TransportConfig;
  try {
    normalizedTransport = normalizeTransportConfig(input.transport);
  } catch (error) {
    return createMCPErrorToolResult(
      `Invalid transport configuration: ${error instanceof Error ? error.message : String(error)}`,
    ) as MCPResult<CreateServerOutput>;
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
    transport: normalizedTransport,
    metadata: {
      description: input.description || '',
    },
  };

  // Try to save (will throw if name exists)
  try {
    await mcpService.save(server);

    // Clear BM25 cache when data changes
    clearBM25Cache();

    const transportType = normalizedTransport.type;
    const summary = [
      `✅ MCP Server Created Successfully`,
      `   Name: ${server.name}`,
      `   ID: ${server.id}`,
      `   Transport: ${transportType}`,
      `   Status: Active`,
      ``,
      `💡 Use "connect_server" to enable this server for the current assistant.`,
    ].join('\n');

    return createMCPStructuredToolResult(summary, {
      server,
      message: `Server "${server.name}" created successfully`,
    });
  } catch (error) {
    return createMCPErrorToolResult(
      `Failed to create server: ${error instanceof Error ? error.message : String(error)}`,
    ) as MCPResult<CreateServerOutput>;
  }
}

async function connectServer(
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
): Promise<MCPResult<unknown>> {
  const input: ConnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
    autoStart: Boolean(args.autoStart ?? true),
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPErrorToolResult(
      'Scope must be "assistant" or "global"',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Validate that at least one identifier is provided
  if (!input.serverId && !input.serverName) {
    return createMCPErrorToolResult(
      'Either serverId or serverName is required',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Find server using helper
  const server = await findServer(mcpService, input.serverId, input.serverName);

  if (!server) {
    const searchTerm = input.serverId || input.serverName || '';
    const suggestions: string[] = [
      'Use search_server to find servers by name',
      'Use list_servers to see all available servers',
      'Check spelling and try exact server name',
    ];

    return new MCPResponseBuilder({
      requestedId: input.serverId,
      requestedName: input.serverName,
      suggestions,
    })
      .withMessage(`Server not found: "${searchTerm}".`)
      .withSuggestions(suggestions)
      .asError(WebMCPErrorCodes.MCP_MANAGER.SERVER_NOT_FOUND);
  }

  // Connect based on scope
  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPErrorToolResult(
        'Assistant context not available. Cannot connect to assistant scope.',
        { tool: 'connect_server', scope: input.scope },
      ) as MCPResult<ConnectServerOutput>;
    }

    const assistant = await assistantService.getById(assistantId);
    if (!assistant) {
      return createMCPErrorToolResult(
        `Assistant not found: ${assistantId}`,
      ) as MCPResult<ConnectServerOutput>;
    }

    // Add to assistant's server list if not already connected
    const serverIds = assistant.mcpServerIds || [];
    if (!serverIds.includes(server.id)) {
      assistant.mcpServerIds = [...serverIds, server.id];
      await assistantService.save(assistant);
    }

    const nextActions = [
      'Use list_servers with filterByAssistant=true to verify connection',
      "Check your assistant's tool list for new capabilities",
      'Server tools are now available for this assistant',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'assistant' as const,
      assistantId,
    })
      .withMessage(
        `Server "${server.name}" connected to assistant "${assistant.name}".\n` +
        `Scope: assistant\n` +
        (input.autoStart
          ? `Status: Starting...`
          : `Status: Registered (manual start required)`),
      )
      .withNextActions(nextActions)
      .asSuccess();
  } else {
    // Global scope: mark server as globally enabled
    server.isActive = true;
    server.updatedAt = new Date();
    await mcpService.save(server);

    const nextActions = [
      'Use list_servers to verify the server is active',
      'Tools from this server are now available system-wide',
      "All assistants can access this server's capabilities",
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'global' as const,
    })
      .withMessage(
        `Server "${server.name}" enabled globally.\n` + `Status: Active`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  }
}

async function disconnectServer(
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
): Promise<MCPResult<unknown>> {
  const input: DisconnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPErrorToolResult(
      'Scope must be "assistant" or "global"',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Validate that at least one identifier is provided
  if (!input.serverId && !input.serverName) {
    return createMCPErrorToolResult(
      'Either serverId or serverName is required',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Find server using helper
  const server = await findServer(mcpService, input.serverId, input.serverName);

  if (!server) {
    const searchTerm = input.serverId || input.serverName || '';
    const suggestions: string[] = [
      'Use search_server to find servers by name',
      'Use list_servers to see all available servers',
      'Check spelling and try exact server name',
    ];

    return new MCPResponseBuilder({
      requestedId: input.serverId,
      requestedName: input.serverName,
      suggestions,
    })
      .withMessage(`Server not found: "${searchTerm}".`)
      .withSuggestions(suggestions)
      .asError(WebMCPErrorCodes.MCP_MANAGER.SERVER_NOT_FOUND);
  }

  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPErrorToolResult('Assistant context not available.', {
        tool: 'disconnect_server',
        scope: input.scope,
      });
    }

    const assistant = await assistantService.getById(assistantId);
    if (!assistant) {
      return createMCPErrorToolResult(`Assistant not found: ${assistantId}`);
    }

    const serverIds = assistant.mcpServerIds || [];
    assistant.mcpServerIds = serverIds.filter((id) => id !== server.id);
    await assistantService.save(assistant);

    const nextActions = [
      'Tools from this server are no longer available to this assistant',
      'Other assistants are not affected',
      'Use connect_server to reconnect anytime',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'assistant' as const,
      impactScope: 'single_assistant' as const,
    })
      .withMessage(
        `Server "${server.name}" disconnected from assistant "${assistant.name}".\n` +
        `Impact: Current assistant only`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  } else {
    server.isActive = false;
    server.updatedAt = new Date();
    await mcpService.save(server);

    const nextActions = [
      'Tools from this server are no longer available',
      'Impact: All assistants system-wide',
      'Server configuration preserved (reconnect anytime with connect_server)',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'global' as const,
      impactScope: 'global' as const,
    })
      .withMessage(
        `Server "${server.name}" disabled globally.\n` + `Impact: System-wide`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  }
}

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
        case 'list_servers':
          return await listServers(mcpService, assistantService, a);
        case 'search_server':
          return await searchServer(mcpService, a);
        case 'create_server':
          return await createServer(mcpService, a);
        case 'connect_server':
          return await connectServer(mcpService, assistantService, a);
        case 'disconnect_server':
          return await disconnectServer(mcpService, assistantService, a);
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
