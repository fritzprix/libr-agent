import type { MCPTool } from '@/lib/mcp-types';

/**
 * Tool schema definitions for the planning server.
 * Simplified flat schemas for Gemini API compatibility.
 */
export const planningTools: MCPTool[] = [
  {
    name: 'create_goal',
    description:
      'Create a single goal for the session. Use when starting a new or complex task.',
    inputSchema: {
      type: 'object',
      properties: {
        goal: {
          type: 'string',
          description:
            'The goal text to set for the session (e.g., "Complete project setup").',
        },
      },
      required: ['goal'],
    },
  },
  {
    name: 'update_goal',
    description:
      'Update the current goal. Use when the goal needs refinement or correction without clearing context.',
    inputSchema: {
      type: 'object',
      properties: {
        goal: {
          type: 'string',
          description: 'The new goal text.',
        },
      },
      required: ['goal'],
    },
  },
  {
    name: 'clear_goal',
    description:
      'Clear the current goal. Use when finishing or abandoning the current goal.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_todo',
    description:
      'Add a todo item to the goal. Use to break down a goal into actionable steps.',
    inputSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description:
            'The name or description of the todo item to add (e.g., "Write documentation").',
        },
        priority: {
          type: 'string',
          enum: ['low', 'medium', 'high'],
          description: 'The priority of the todo item.',
        },
        dependsOn: {
          type: 'array',
          items: { type: 'number' },
          description: 'List of todo IDs that this todo depends on.',
        },
      },
      required: ['name'],
    },
  },
  {
    name: 'update_todo',
    description:
      'Update an existing todo item. Use to refine task details, change priority, or update dependencies. You can specify either id (database ID) or index (0-based position in the list).',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 1,
          description: 'The database ID of the todo to update.',
        },
        index: {
          type: 'number',
          minimum: 0,
          description:
            'The 0-based index position of the todo in the current list.',
        },
        name: {
          type: 'string',
          description: 'The new name/description of the todo.',
        },
        status: {
          type: 'string',
          enum: ['pending', 'completed', 'blocked'],
          description: 'The new status of the todo.',
        },
        priority: {
          type: 'string',
          enum: ['low', 'medium', 'high'],
          description: 'The new priority of the todo.',
        },
        dependsOn: {
          type: 'array',
          items: { type: 'number' },
          description: 'The new list of dependencies.',
        },
      },
    },
  },
  {
    name: 'mark_todo',
    description:
      'Mark a todo item as completed or pending, optionally with a completion summary. You can specify either id (database ID) or index (0-based position in the list).',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 1,
          description: 'The database ID of the todo to update',
        },
        index: {
          type: 'number',
          minimum: 0,
          description:
            'The 0-based index position of the todo in the current list',
        },
        completed: {
          type: 'boolean',
          description:
            'Whether to mark the todo as completed (true) or pending (false). Defaults to true.',
        },
        summary: {
          type: 'string',
          description:
            'Optional summary or completion note for the todo (e.g., "Completed with PR #42").',
        },
      },
    },
  },
  {
    name: 'clear_todos',
    description:
      'Clear specific todos by their IDs, or all todos if no IDs are provided. Use to remove completed tasks or reset the todo list.',
    inputSchema: {
      type: 'object',
      properties: {
        ids: {
          type: 'array',
          items: { type: 'number', minimum: 1 },
          description:
            'Array of todo IDs to clear. If not provided or empty, all todos will be cleared.',
        },
      },
    },
  },
  {
    name: 'clear_session',
    description:
      'Clear all session state (goal, todos, and scratchpad items). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_scratchpad',
    description:
      'Add a note to your Scratchpad (Working Memory). Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.\n\nOptional source parameter: Provide the source of information for citation tracking (e.g., URLs, file paths, or tool result IDs like "https://example.com/article" or "file://path/to/doc.txt").',
    inputSchema: {
      type: 'object',
      properties: {
        note: {
          type: 'string',
          description:
            'The content to add to the scratchpad (e.g., "User requested feature X", "File path: src/main.ts").',
        },
        source: {
          type: 'string',
          description:
            'Optional source of the information for citation tracking. Examples: "https://example.com/article", "file://workspace/docs/readme.md", "tool_result_id:abc123"',
        },
      },
      required: ['note'],
    },
  },
  {
    name: 'clear_scratchpad',
    description:
      'Remove a note from your Scratchpad. Use this to clear information that is no longer relevant to free up context window space.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 0,
          description: 'The ID of the scratchpad item to clear.',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'get_current_state',
    description:
      'Get current planning state including Goal, Todos, and Scratchpad as structured JSON data for UI visualization',
    inputSchema: {
      type: 'object',
      properties: {
        include_completed: {
          type: 'boolean',
          description:
            'Whether to include completed todos in the output. Defaults to true.',
        },
        include_scratchpad: {
          type: 'boolean',
          description:
            'Whether to include scratchpad items in the output. Defaults to true.',
        },
      },
    },
  },
  {
    name: 'sequentialthinking',
    description:
      'Sequential thinking tool for multi-step reflective problem solving. Accepts a thought payload and maintains per-session thought history and branches.',
    inputSchema: {
      type: 'object',
      properties: {
        thought: { type: 'string', description: 'Your current thinking step' },
        nextThoughtNeeded: {
          type: 'boolean',
          description: 'Whether another thought step is needed',
        },
        thoughtNumber: { type: 'integer', minimum: 1 },
        totalThoughts: { type: 'integer', minimum: 1 },
        isRevision: { type: 'boolean' },
        revisesThought: { type: 'integer', minimum: 1 },
        branchFromThought: { type: 'integer', minimum: 1 },
        branchId: { type: 'string' },
        needsMoreThoughts: { type: 'boolean' },
        category: {
          type: 'string',
          description:
            'The category of the thought (e.g., "hypothesis", "planning", "reflection").',
        },
        relatedTodoId: {
          type: 'integer',
          description: 'The ID of the todo item related to this thought.',
        },
        nextAction: {
          type: 'string',
          description: 'The next action to take based on this thought.',
        },
      },
      required: [
        'thought',
        'nextThoughtNeeded',
        'thoughtNumber',
        'totalThoughts',
      ],
    },
  },
];
