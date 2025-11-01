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
      },
      required: ['name'],
    },
  },
  {
    name: 'mark_todo',
    description:
      'Mark a todo item as completed or pending by its ID, optionally with a completion summary.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 1,
          description: 'The ID of the todo to update',
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
      required: ['id'],
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
      'Clear all session state (goal, todos, and notes). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_memo',
    description:
      'Add a memo to the session. Memos are temporary records, observations, or context information. Each memo is assigned a unique ID for reference.',
    inputSchema: {
      type: 'object',
      properties: {
        memo: {
          type: 'string',
          description:
            'The memo text to add (e.g., "User requested feature X").',
        },
      },
      required: ['memo'],
    },
  },
  {
    name: 'clear_memo',
    description: 'Clear a memo from the session by its ID.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 0,
          description: 'The ID of the memo to clear.',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'get_current_state',
    description:
      'Get current planning state as structured JSON data for UI visualization',
    inputSchema: { type: 'object', properties: {} },
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
