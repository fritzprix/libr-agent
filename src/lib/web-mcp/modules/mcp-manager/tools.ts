import type { MCPTool } from '@/lib/mcp-types';
import {
  createStringSchema,
  createIntegerSchema,
  createObjectSchema,
  createBooleanSchema,
} from '@/lib/mcp-types';

export const mcpManagerTools: MCPTool[] = [
  {
    name: 'list_servers',
    description:
      'List all registered MCP servers with pagination and filtering options',
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
      'Search MCP servers by name, description, or tags with fuzzy matching and relevance sorting',
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
          description: 'Number of items per page (default: 20, use -1 for all)',
          minimum: -1,
        }),
        byNameOnly: createBooleanSchema({
          description:
            'Search only in server names (default: true). Set to false to search in descriptions and tags as well.',
        }),
        includeInactive: createBooleanSchema({
          description:
            'Include inactive servers in the results (default: true)',
        }),
      },
      required: ['query'],
      additionalProperties: false,
    }),
  },
  {
    name: 'create_server',
    description:
      'Create a new MCP server configuration with transport settings (stdio or http)',
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
          description: 'Transport configuration (stdio or http)',
          properties: {
            type: createStringSchema({
              description: 'Transport type: "stdio" or "http"',
            }),
            // stdio fields
            command: createStringSchema({
              description: 'Command to execute (required for stdio)',
            }),
            args: {
              type: 'array',
              items: { type: 'string' },
              description: 'Command arguments (optional for stdio)',
            },
            env: {
              type: 'object',
              additionalProperties: { type: 'string' },
              description: 'Environment variables (optional for stdio)',
            },
            // http fields
            url: createStringSchema({
              description: 'HTTP endpoint URL (required for http)',
            }),
            headers: {
              type: 'object',
              additionalProperties: { type: 'string' },
              description: 'HTTP headers (optional for http)',
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
      'Connect a server to the current assistant or enable it globally for all assistants',
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
            'Connection scope: "assistant" (current assistant only) or "global" (all assistants). Default: "assistant"',
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
      'Disconnect a server from the current assistant or disable it globally',
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
            'Scope to disconnect from: "assistant" or "global". Default: "assistant"',
        }),
      },
      additionalProperties: false,
    }),
  },
];
