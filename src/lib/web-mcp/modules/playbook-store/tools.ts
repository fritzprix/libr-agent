import type { MCPTool } from '@/lib/mcp-types';

// Tool schema declarations for the Playbook WebMCP server.
// Handlers are implemented in store.ts to keep access to local state and helpers.

export const playbookTools: MCPTool[] = [
  {
    name: 'create_playbook',
    description:
      'Create a new reusable playbook template with parameterized inputs. Supports {{variableName}} templates in descriptions and purposes.',
    inputSchema: {
      type: 'object',
      properties: {
        title: {
          type: 'string',
          description:
            'Short, user-facing title for this playbook (e.g., "Company Financial Analysis")',
        },
        goal: {
          type: 'string',
          description:
            'Goal description. Supports {{variableName}} templates (e.g., "Analyze {{targetCompany}}\'s financials")',
        },
        inputs: {
          type: 'array',
          description: 'Input parameters required to execute this playbook',
          items: {
            type: 'object',
            properties: {
              name: {
                type: 'string',
                description: 'Variable name (e.g., "targetCompany")',
              },
              description: {
                type: 'string',
                description: 'Human-readable description of this input',
              },
              type: {
                type: 'string',
                enum: ['string', 'number', 'boolean'],
                description: 'Data type of the input value',
              },
              defaultValue: {
                type: 'string',
                description: 'Optional default value',
              },
            },
            required: ['name', 'description'],
          },
        },
        workflow: {
          type: 'array',
          description:
            'An ordered list of steps (PlaybookStep) that make up this workflow. Descriptions and purposes support {{variableName}} templates.',
          items: {
            type: 'object',
            properties: {
              stepId: { type: 'string', description: 'Unique id for the step' },
              description: {
                type: 'string',
                description: 'Human readable description of this step',
              },
              action: {
                type: 'object',
                properties: {
                  toolName: {
                    type: 'string',
                    description: 'The tool to invoke for this step',
                  },
                  purpose: {
                    type: 'string',
                    description:
                      'High-level purpose for invoking the tool (agent will configure params)',
                  },
                },
                required: ['toolName', 'purpose'],
              },
              requiredData: {
                type: 'array',
                items: { type: 'string' },
                description: 'List of data keys required by this step',
              },
              outputVariable: {
                type: 'string',
                description:
                  'Name used to reference this step output in later steps',
              },
            },
            required: [
              'description',
              'action',
              'requiredData',
              'outputVariable',
            ],
          },
        },
        successCriteria: {
          type: 'object',
          description:
            'Objective criteria describing when the playbook is considered successful',
          properties: {
            description: {
              type: 'string',
              description: 'Human readable success description',
            },
            requiredArtifacts: {
              type: 'array',
              items: { type: 'string' },
              description: 'Files that must be produced for success',
            },
          },
          required: ['description'],
        },
        // Legacy/Optional fields
        agentId: {
          type: 'string',
          description: 'Optional agent ID for playbook ownership',
        },
        initialCommand: {
          type: 'string',
          description: 'Optional example command that inspired this playbook',
        },
      },
      required: ['title', 'goal', 'inputs', 'workflow'],
    },
  },
  {
    name: 'select_playbook',
    description:
      'Select a playbook by id and return detailed formatted text + agent prompt to execute it',
    inputSchema: {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The playbook id to select' },
      },
      required: ['id'],
    },
  },
  {
    name: 'list_playbooks',
    description:
      'List playbooks with optional paging (text-only, non-interrupting for agent autonomous use)',
    inputSchema: {
      type: 'object',
      properties: {
        page: {
          type: 'number',
          description: 'Page number to retrieve (1-based)',
        },
        pageSize: {
          type: 'number',
          description: 'Number of items per page; -1 for all',
        },
      },
    },
  },
  {
    name: 'show_playbooks',
    description:
      'Display playbooks with interactive UI (includes HTML UI resource for frontend, pauses agent)',
    inputSchema: {
      type: 'object',
      properties: {
        page: {
          type: 'number',
          description: 'Page number to retrieve (1-based)',
        },
        pageSize: {
          type: 'number',
          description: 'Number of items per page; -1 for all',
        },
      },
    },
  },
  {
    name: 'get_playbook_page',
    description:
      'Navigate to a specific page of playbooks with interactive UI (for pagination buttons, pauses agent)',
    inputSchema: {
      type: 'object',
      properties: {
        page: {
          type: 'number',
          description: 'Page number to navigate to (1-based)',
        },
        pageSize: {
          type: 'number',
          description:
            'Number of items per page (optional, keeps previous size)',
        },
      },
      required: ['page'],
    },
  },
  {
    name: 'delete_playbook',
    description: 'Delete a playbook by id',
    inputSchema: {
      type: 'object',
      properties: { id: { type: 'string' } },
      required: ['id'],
    },
  },
  {
    name: 'get_playbook',
    description:
      'Get detailed information for a single playbook by id (formatted text + structured)',
    inputSchema: {
      type: 'object',
      properties: {
        id: { type: 'string', description: 'The playbook id to retrieve' },
      },
      required: ['id'],
    },
  },
  {
    name: 'update_playbook',
    description: 'Update a playbook by id',
    inputSchema: {
      type: 'object',
      properties: {
        id: { type: 'string' },
        playbook: {
          type: 'object',
          properties: {
            title: { type: 'string' },
            agentId: { type: 'string' },
            goal: { type: 'string' },
            inputs: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  name: { type: 'string' },
                  description: { type: 'string' },
                  type: {
                    type: 'string',
                    enum: ['string', 'number', 'boolean'],
                  },
                  defaultValue: { type: 'string' },
                },
                required: ['name', 'description'],
              },
            },
            initialCommand: { type: 'string' },
            workflow: {
              type: 'array',
              items: {
                type: 'object',
                properties: {
                  description: { type: 'string' },
                  action: {
                    type: 'object',
                    properties: {
                      toolName: { type: 'string' },
                      purpose: { type: 'string' },
                    },
                    required: ['toolName', 'purpose'],
                  },
                  requiredData: { type: 'array', items: { type: 'string' } },
                  outputVariable: { type: 'string' },
                },
                required: [
                  'description',
                  'action',
                  'requiredData',
                  'outputVariable',
                ],
              },
            },
            successCriteria: {
              type: 'object',
              properties: {
                description: { type: 'string' },
                requiredArtifacts: { type: 'array', items: { type: 'string' } },
              },
            },
          },
        },
      },
      required: ['id'],
    },
  },
];

export default playbookTools;
