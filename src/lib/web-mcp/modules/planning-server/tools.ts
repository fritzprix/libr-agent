import type { MCPTool } from '@/lib/mcp-types';

/**
 * Tool schema definitions for the planning server.
 * Simplified flat schemas for Gemini API compatibility.
 */
export const planningTools: MCPTool[] = [
  {
    name: 'createGoal',
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
    name: 'updateGoal',
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
    name: 'clearGoal',
    description:
      'Clear the current goal. Use when finishing or abandoning the current goal.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'addTodo',
    description:
      'Add a todo item to the goal. Use to break down a goal into actionable steps.',
    inputSchema: {
      type: 'object',
      properties: {
        title: {
          type: 'string',
          description:
            'Short summary of the task (e.g., "Write documentation").',
        },
        description: {
          type: 'string',
          description: 'Detailed instructions or context for the task.',
        },
        priority: {
          type: 'string',
          enum: ['low', 'medium', 'high'],
          description: 'The priority of the todo item.',
        },
      },
      required: ['title'],
    },
  },
  {
    name: 'checkTodo',
    description:
      'Mark a todo item as checked (completed) or unchecked, optionally with a completion summary. You can specify either id (database ID) or index (0-based position in the list).',
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
        checked: {
          type: 'boolean',
          description:
            'Whether to mark the todo as checked (true) or unchecked (false). Defaults to true.',
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
    name: 'clearTodos',
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
    name: 'clearSession',
    description:
      'Clear all session state (goal, todos, and scratchpad items). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'addScratchpad',
    description:
      'Add a note to your Scratchpad (Working Memory). Content here is ALWAYS visible in your context. Use this for keeping track of important findings, file paths, IDs, or intermediate analysis results that you need to reference frequently during the task.\n\nOptional source parameter: Provide the source of information for citation tracking (e.g., URLs, file paths, or tool result IDs like "https://example.com/article" or "file://path/to/doc.txt").',
    inputSchema: {
      type: 'object',
      properties: {
        title: {
          type: 'string',
          description:
            'Optional title for the note. Helps in identifying the note in the list.',
        },
        note: {
          type: 'string',
          description:
            'The content to add to the scratchpad (e.g., "User requested feature X", "File path: src/main.ts").',
        },
        tags: {
          type: 'array',
          items: { type: 'string' },
          description: 'Optional tags for categorization and filtering.',
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
    name: 'readScratchpad',
    description:
      'Read specific scratchpad items by their IDs or filter by tags. Use this to retrieve the full content of scratchpad items when they are not fully visible in the context.',
    inputSchema: {
      type: 'object',
      properties: {
        ids: {
          type: 'array',
          items: { type: 'number', minimum: 0 },
          description: 'List of scratchpad IDs to read.',
        },
        tags: {
          type: 'array',
          items: { type: 'string' },
          description: 'List of tags to filter by.',
        },
      },
    },
  },
  {
    name: 'clearScratchpad',
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
    name: 'getCurrentState',
    description:
      'Get current planning state including Goal, Todos, and Scratchpad as structured JSON data for UI visualization',
    inputSchema: {
      type: 'object',
      properties: {
        include_checked: {
          type: 'boolean',
          description:
            'Whether to include checked todos in the output. Defaults to true.',
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
    name: 'pauseAndThink',
    description:
      'Pause to think about the problem, plan your approach, or analyze results before taking action. Use this when you need to reason through complex decisions or maintain context. Simpler alternative to sequentialthinking.',
    inputSchema: {
      type: 'object',
      properties: {
        thought: {
          type: 'string',
          description:
            'Your current thought, analysis, or plan. Be clear and specific about what you are thinking through.',
        },
        nextAction: {
          type: 'string',
          description:
            'Optional: The specific next action you plan to take after this thought. Helps maintain continuity.',
        },
      },
      required: ['thought'],
    },
  },
  {
    name: 'critiqueAndReflection',
    description:
      'Reflect on the current state and provide a critique of the progress. Use this tool to pause, analyze what has been done, identify potential issues or missed steps, and plan the next actions carefully.',
    inputSchema: {
      type: 'object',
      properties: {
        critique: {
          type: 'string',
          description: 'A critical evaluation of the results achieved so far.',
        },
        reflection: {
          type: 'string',
          description:
            'Self-reflection on any shortcomings or areas for improvement in the process.',
        },
        nextAction: {
          type: 'string',
          description: 'The expected next action based on the reflection.',
        },
      },
      required: ['critique', 'reflection', 'nextAction'],
    },
  },
];
