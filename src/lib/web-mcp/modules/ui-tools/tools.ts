/**
 * UI Tools Schema Declarations
 *
 * Defines the MCP tool schemas for user interaction tools
 */

import type { MCPTool } from '@/lib/mcp-types';

/**
 * UI Tools Schema with usage examples
 *
 * Example usage:
 * 1. Text prompt:
 *    { prompt: "What is your name?", type: "text" }
 * 2. Single select:
 *    { prompt: "Choose color", type: "select", options: ["Red", "Blue"] }
 * 3. Multi-select:
 *    { prompt: "Select features", type: "multiselect", options: ["A", "B", "C"] }
 * 4. Bar chart:
 *    { type: "bar", data: [{label: "Jan", value: 100}, {label: "Feb", value: 150}], title: "Sales" }
 * 5. Wait UI:
 *    { message: "Processing...", resumeInstruction: "Continue after data processing" }
 */
export const uiToolsSchema: MCPTool[] = [
  // Migration note: Previously used context object with {reason, command, nextAction}.
  // Now simplified to direct resumeInstruction parameter for both tools.
  // Callers should pass resumeInstruction directly instead of wrapping in context object.
  {
    name: 'prompt_user',
    description:
      'Display an interactive prompt to the user (text input, select, or multiselect). Use this to gather user input interactively.',
    inputSchema: {
      type: 'object',
      properties: {
        prompt: {
          type: 'string',
          description: 'The question or instruction to show the user',
        },
        type: {
          type: 'string',
          enum: ['text', 'select', 'multiselect'],
          description: 'Type of prompt UI to display',
        },
        options: {
          type: 'array',
          items: { type: 'string' },
          description:
            'Options for select/multiselect (required for those types)',
        },
      },
      required: ['prompt', 'type'],
    },
  },
  {
    name: 'reply_prompt',
    description:
      'Receive user response from prompt UI (automatically called by UI action)',
    inputSchema: {
      type: 'object',
      properties: {
        messageId: {
          type: 'string',
          description: 'ID of the prompt being replied to',
        },
        answer: {
          description:
            'User answer: string for text/select, array of strings for multiselect, null if cancelled',
        },
        cancelled: {
          type: 'boolean',
          description: 'Whether the user cancelled the prompt',
        },
      },
      required: ['messageId'],
    },
  },
  {
    name: 'visualize_data',
    description:
      'Create a simple data visualization (bar or line chart). Useful for presenting numeric data in a visual format. Maximum 20 data points recommended for readability.',
    inputSchema: {
      type: 'object',
      properties: {
        type: {
          type: 'string',
          enum: ['bar', 'line'],
          description:
            'Type of chart to create: "bar" for comparisons, "line" for trends',
        },
        data: {
          type: 'array',
          items: {
            type: 'object',
            properties: {
              label: {
                type: 'string',
                description: 'Label for this data point (must be non-empty)',
              },
              value: {
                type: 'number',
                description: 'Numeric value (must be finite, non-NaN)',
              },
            },
            required: ['label', 'value'],
          },
          description: 'Data points to visualize (at least 1 required)',
          minItems: 1,
          maxItems: 50,
        },
        title: {
          type: 'string',
          description:
            'Optional title for the chart (displayed above the visualization)',
        },
      },
      required: ['type', 'data'],
    },
  },
  {
    name: 'wait_for_user_resume',
    description:
      'Display wait UI with continue button for long operations, especially useful when repetitive polling is needed',
    inputSchema: {
      type: 'object',
      properties: {
        message: {
          type: 'string',
          description: 'Message to display to user',
        },
        resumeInstruction: {
          type: 'string',
          description: 'What to do after the user resumes (for agent context)',
        },
      },
      required: ['message', 'resumeInstruction'],
    },
  },
  {
    name: 'resume_from_wait',
    description: 'Resume from wait (called by UI button click)',
    inputSchema: {
      type: 'object',
      properties: {
        resumeInstruction: {
          type: 'string',
          description: 'Resume instruction that was set when waiting started',
        },
        startedAt: {
          type: 'string',
          description: 'ISO 8601 timestamp when waiting started',
        },
        sessionId: {
          type: 'string',
          description: 'Optional session ID for validation',
        },
      },
      required: ['resumeInstruction', 'startedAt'],
    },
  },
];
