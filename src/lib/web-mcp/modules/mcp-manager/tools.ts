import type { MCPTool } from '@/lib/mcp-types';
import {
  createStringSchema,
  createIntegerSchema,
  createObjectSchema,
  createBooleanSchema,
  createEnumSchema,
} from '@/lib/mcp-types';

export const mcpManagerTools: MCPTool[] = [
  {
    name: 'list_servers',
    description:
      'List all registered MCP servers with pagination and filtering. Use this when you need to: view all available servers, check which servers are connected to current assistant, review server configurations before connecting/disconnecting, or audit server inventory.',
    inputSchema: createObjectSchema({
      description: 'Pagination and filter options for listing MCP servers',
      properties: {
        page: createIntegerSchema({
          description: 'Page number (1-based, default: 1)',
          minimum: 1,
        }),
        pageSize: createIntegerSchema({
          description: 'Number of items per page (default: 20, use -1 for all)',
          minimum: -1,
        }),
        filterByAssistant: createBooleanSchema({
          description:
            'Filter to only show servers connected to the current assistant (default: false)',
        }),
        includeInactive: createBooleanSchema({
          description:
            'Include inactive servers in the results (default: true)',
        }),
      },
      additionalProperties: false,
    }),
  },
  {
    name: 'search_server',
    description:
      'Search MCP servers by name, description, or tags with intelligent ranking. Use this when you need to: find a specific server by partial name or keyword, discover servers with particular capabilities, or quickly locate servers in large inventory. BM25 mode (default) provides relevance-based ranking best for natural language queries. Simple mode offers fast substring matching best for exact name searches.',
    inputSchema: createObjectSchema({
      description: 'Search query and options',
      properties: {
        query: createStringSchema({
          description:
            'Search query string (searches in name, description, and tags)',
          minLength: 1,
        }),
        page: createIntegerSchema({
          description: 'Page number (1-based, default: 1)',
          minimum: 1,
        }),
        pageSize: createIntegerSchema({
          description: 'Items per page (default: 20, -1 for all)',
          minimum: -1,
        }),
        searchMode: createEnumSchema(['bm25', 'simple'], {
          description:
            'Search mode: "bm25" for ranked relevance search (default), "simple" for basic substring matching',
          default: 'bm25',
        }),
        byNameOnly: createBooleanSchema({
          description:
            'Simple mode only: search only server names (default: true). Ignored in BM25 mode.',
        }),
        includeInactive: createBooleanSchema({
          description:
            'Include inactive servers in the results (default: true)',
        }),
        weights: createObjectSchema({
          description:
            'BM25 mode only: field weights for relevance scoring. Higher weight = more important.',
          properties: {
            nameWeight: createIntegerSchema({
              description: 'Weight for name field (default: 2.0)',
              minimum: 0,
            }),
            descWeight: createIntegerSchema({
              description: 'Weight for description field (default: 1.0)',
              minimum: 0,
            }),
          },
        }),
      },
      required: ['query'],
      additionalProperties: false,
    }),
  },
  {
    name: 'create_server',
    description:
      'Register a new MCP server configuration in the system. Use this when you need to: add a new external MCP server to available servers list, configure connection settings for an MCP server (stdio command or HTTP endpoint), or make a server discoverable for assistants. Note: Creating a server only registers it in the database. Use "connect_server" afterwards to enable it for an assistant.',
    inputSchema: createObjectSchema({
      description: 'Server configuration parameters',
      properties: {
        name: createStringSchema({
          description: 'Unique server name (case-insensitive)',
          minLength: 1,
        }),
        description: createStringSchema({
          description: 'Server description for documentation',
        }),
        transport: createObjectSchema({
          description:
            'Transport configuration: "stdio" for local command execution or "http" for remote endpoint. Choose stdio for local servers/processes (e.g., npx commands), http for remote APIs/services.',
          properties: {
            type: createStringSchema({
              description:
                'Transport type: "stdio" (local command execution) or "http" (remote HTTP endpoint)',
            }),
            // stdio fields
            command: createStringSchema({
              description:
                'Command to execute (required for stdio transport, e.g., "node", "npx", "python")',
            }),
            args: {
              type: 'array',
              items: { type: 'string' },
              description:
                'Command arguments (optional for stdio, e.g., ["server.js", "--port", "3000"])',
            },
            env: {
              type: 'object',
              additionalProperties: { type: 'string' },
              description:
                'Environment variables as an object (NOT a JSON string). Example: {"API_KEY": "xxx", "DEBUG": "true"}. Do NOT use string format like \'{"key": "value"}\'.',
            },
            // http fields
            url: createStringSchema({
              description:
                'HTTP endpoint URL (required for http transport, e.g., "https://api.example.com/mcp")',
            }),
            headers: {
              type: 'object',
              additionalProperties: { type: 'string' },
              description:
                'HTTP headers as an object (NOT a JSON string). Example: {"Authorization": "Bearer token", "X-API-Key": "xxx"}. Do NOT use string format.',
            },
          },
          required: ['type'],
        }),
        tags: {
          type: 'array',
          items: { type: 'string' },
          description: 'Tags for categorization and search',
        },
      },
      required: ['name', 'transport'],
      additionalProperties: false,
    }),
  },
  {
    name: 'connect_server',
    description:
      'Enable an MCP server for use by adding it to an assistant or globally. Use this when you need to: add tools from a specific server to current assistant (scope: "assistant"), make a server available to all assistants system-wide (scope: "global"), or activate a previously created/disconnected server. Assistant scope: tools only available to current assistant. Global scope: tools available to all assistants.',
    inputSchema: createObjectSchema({
      description: 'Connection options',
      properties: {
        serverId: createStringSchema({
          description:
            'Server ID to connect (either serverId or serverName required)',
        }),
        serverName: createStringSchema({
          description:
            'Server name to connect (either serverId or serverName required)',
        }),
        scope: createStringSchema({
          description:
            'Connection scope: "assistant" (tools available only to current assistant) or "global" (tools available to all assistants system-wide). Default: "assistant"',
        }),
        autoStart: createBooleanSchema({
          description:
            'Automatically start the server if not running (default: true, currently placeholder)',
        }),
      },
      additionalProperties: false,
    }),
  },
  {
    name: 'disconnect_server',
    description:
      'Disable an MCP server by removing it from an assistant or globally. Use this when you need to: remove tools from current assistant without deleting the server configuration, disable a server system-wide for all assistants, or deactivate a problematic server. Assistant scope: removes server only from current assistant (other assistants keep access). Global scope: removes server from all assistants. Note: Disconnecting does NOT delete the server configuration - use list_servers or search_server to verify disconnection.',
    inputSchema: createObjectSchema({
      description: 'Disconnection options',
      properties: {
        serverId: createStringSchema({
          description:
            'Server ID to disconnect (either serverId or serverName required)',
        }),
        serverName: createStringSchema({
          description:
            'Server name to disconnect (either serverId or serverName required)',
        }),
        scope: createStringSchema({
          description:
            'Scope to disconnect from: "assistant" (current assistant only) or "global" (all assistants). Default: "assistant"',
        }),
      },
      additionalProperties: false,
    }),
  },
];
