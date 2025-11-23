import type { MCPTool } from '@/lib/mcp-types';
import {
  createStringSchema,
  createObjectSchema,
  createArraySchema,
  createNumberSchema,
} from '@/lib/mcp-types';

export const assistantManagerTools: MCPTool[] = [
  {
    name: 'list_assistants',
    description: 'List available assistants with pagination support.',
    inputSchema: createObjectSchema({
      properties: {
        page: createNumberSchema({
          description: 'Page number (default: 1)',
        }),
        pageSize: createNumberSchema({
          description: 'Items per page (default: 20)',
        }),
      },
    }),
  },
  {
    name: 'get_assistant',
    description: 'Get details of a specific assistant by ID.',
    inputSchema: createObjectSchema({
      properties: {
        id: createStringSchema({ description: 'The ID of the assistant' }),
      },
      required: ['id'],
    }),
  },
  {
    name: 'create_assistant',
    description: 'Create a new assistant.',
    inputSchema: createObjectSchema({
      properties: {
        name: createStringSchema({ description: 'Name of the assistant' }),
        systemPrompt: createStringSchema({
          description: 'System prompt for the assistant',
        }),
        description: createStringSchema({
          description: 'Description of the assistant',
        }),
        mcpServerIds: createArraySchema({
          items: createStringSchema({ description: 'ID of MCP server' }),
          description: 'List of MCP server IDs to enable for this assistant',
        }),
        allowedBuiltInServiceAliases: createArraySchema({
          items: createStringSchema({
            description: 'Alias of built-in service',
          }),
          description: 'List of allowed built-in service aliases',
        }),
      },
      required: ['name', 'systemPrompt'],
    }),
  },
  {
    name: 'update_assistant',
    description: 'Update an existing assistant.',
    inputSchema: createObjectSchema({
      properties: {
        id: createStringSchema({
          description: 'The ID of the assistant to update',
        }),
        name: createStringSchema({ description: 'Name of the assistant' }),
        systemPrompt: createStringSchema({
          description: 'System prompt for the assistant',
        }),
        description: createStringSchema({
          description: 'Description of the assistant',
        }),
        mcpServerIds: createArraySchema({
          items: createStringSchema({ description: 'ID of MCP server' }),
          description: 'List of MCP server IDs to enable for this assistant',
        }),
        allowedBuiltInServiceAliases: createArraySchema({
          items: createStringSchema({
            description: 'Alias of built-in service',
          }),
          description: 'List of allowed built-in service aliases',
        }),
      },
      required: ['id'],
    }),
  },
  {
    name: 'delete_assistant',
    description: 'Delete an assistant.',
    inputSchema: createObjectSchema({
      properties: {
        id: createStringSchema({
          description: 'The ID of the assistant to delete',
        }),
      },
      required: ['id'],
    }),
  },
  {
    name: 'search_assistant',
    description:
      'Search assistants using BM25 ranking algorithm based on name, description, and system prompt.',
    inputSchema: createObjectSchema({
      properties: {
        query: createStringSchema({
          description: 'Search query to match against assistant fields',
        }),
        limit: createNumberSchema({
          description: 'Maximum number of results to return (default: 10)',
        }),
      },
      required: ['query'],
    }),
  },
];
