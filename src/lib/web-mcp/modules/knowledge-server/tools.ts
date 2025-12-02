import type { MCPTool } from '@/lib/mcp-types';

export const knowledgeTools: MCPTool[] = [
  {
    name: 'save_knowledge',
    description:
      'Save a piece of knowledge (code snippet, rule, preference, etc.) to the long-term memory.',
    inputSchema: {
      type: 'object',
      properties: {
        title: {
          type: 'string',
          description: 'A short, descriptive title for the knowledge item.',
        },
        content: {
          type: 'string',
          description: 'The actual content/information to store.',
        },
        tags: {
          type: 'array',
          items: { type: 'string' },
          description: 'Tags for categorization and easier retrieval.',
        },
      },
      required: ['title', 'content'],
    },
  },
  {
    name: 'search_knowledge',
    description:
      'Search for knowledge items by query string (matches title/content) and/or tags.',
    inputSchema: {
      type: 'object',
      properties: {
        query: {
          type: 'string',
          description: 'Text to search for in titles and content.',
        },
        tags: {
          type: 'array',
          items: { type: 'string' },
          description: 'Filter results by these tags.',
        },
      },
    },
  },
  {
    name: 'read_knowledge',
    description: 'Retrieve the full content of a specific knowledge item.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          description: 'The ID of the knowledge item to read.',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'list_knowledge',
    description:
      'List all knowledge items for the current assistant, sorted by most recently updated.',
    inputSchema: {
      type: 'object',
      properties: {
        limit: {
          type: 'number',
          description: 'Maximum number of items to return (default: 50).',
        },
        offset: {
          type: 'number',
          description: 'Number of items to skip (default: 0).',
        },
      },
    },
  },
  {
    name: 'delete_knowledge',
    description: 'Delete a knowledge item.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          description: 'The ID of the knowledge item to delete.',
        },
      },
      required: ['id'],
    },
  },
];
