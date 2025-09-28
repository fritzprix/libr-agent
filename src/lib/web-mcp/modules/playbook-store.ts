import {
  createMCPStructuredResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, MCPTool, WebMCPServer } from '@/lib/mcp-types';
import type { Playbook } from '@/types/playbook';
import { dbService } from '@/lib/db';

/**
 * In-memory fallback when DB is not available in worker environment.
 */
interface PlaybookRecord extends Playbook {
  id: string;
  createdAt?: string | Date;
  updatedAt?: string | Date;
}

let nextId = 1;
const inMemory: PlaybookRecord[] = [];

// Context state for assistant ID
let currentAssistantId: string | null = null;

function formatPlaybook(p: PlaybookRecord): string {
  const created = p.createdAt ? String(p.createdAt) : 'unknown';
  const steps = Array.isArray(p.workflow) ? p.workflow.length : 0;
  return `id:${p.id} goal:"${p.goal}" initial:"${p.initialCommand || ''}" steps:${steps} createdAt:${created}`;
}

const tools: MCPTool[] = [
  {
    name: 'create_playbook',
    description: 'Create a new playbook (workflow)',
    inputSchema: {
      type: 'object',
      properties: {
        goal: {
          type: 'string',
          description:
            'Short, user-facing description of the intended goal this playbook achieves',
        },
        initialCommand: {
          type: 'string',
          description:
            "The user's original natural-language command that spawned this playbook",
        },
        workflow: {
          type: 'array',
          description:
            'An ordered list of steps (PlaybookStep) that make up this workflow',
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
      },
      required: ['goal', 'workflow'],
    },
  },
  {
    name: 'list_playbooks',
    description: 'List playbooks with optional paging and filters',
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
            agentId: { type: 'string' },
            goal: { type: 'string' },
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

const playbookStore: WebMCPServer = {
  name: 'playbook',
  version: '0.1.0',
  description:
    'Persisted playbook store for agent workflows (create/list/update/delete)',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    const hasDB = !!dbService.playbooks;

    try {
      switch (name) {
        case 'create_playbook': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              'Assistant ID not set. Please set context first.',
            );
          }

          const id = String(Date.now()) + '-' + String(nextId++);
          const playbook: PlaybookRecord = {
            id,
            agentId: currentAssistantId,
            goal: String(a.goal || ''),
            initialCommand: String(a.initialCommand || ''),
            workflow: ((a.workflow as Playbook['workflow']) || []).map(
              (s, i) => ({
                stepId: s?.stepId ?? `${id}-step-${i + 1}`,
                description: s?.description ?? '',
                action: s?.action ?? { toolName: '', purpose: '' },
                requiredData: s?.requiredData ?? [],
                outputVariable: s?.outputVariable ?? '',
              }),
            ),
            successCriteria:
              (a.successCriteria as Playbook['successCriteria']) || {
                description: '',
              },
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
          };

          if (hasDB && dbService.playbooks) {
            await dbService.playbooks.upsert(playbook as PlaybookRecord);
            const saved = await dbService.playbooks.read(id);
            const formatted = saved
              ? formatPlaybook(saved as PlaybookRecord)
              : `Playbook ${id} created`;
            return createMCPStructuredResponse(formatted, {
              success: true,
              playbook: saved,
            });
          }

          inMemory.push(playbook);
          const formattedInMemory = formatPlaybook(playbook);
          return createMCPStructuredResponse(formattedInMemory, {
            success: true,
            playbook,
          });
        }

        case 'list_playbooks': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              'Assistant ID not set. Please set context first.',
            );
          }

          const page = Number(a.page || 1);
          const pageSize = Number(a.pageSize || -1);

          if (hasDB && dbService.playbooks) {
            const pageResult = await dbService.playbooks.getPage(
              page,
              pageSize,
            );
            let items = pageResult.items as PlaybookRecord[];
            items = items.filter((p) => p.agentId === currentAssistantId);

            if (!items || items.length === 0) {
              return createMCPStructuredResponse('No playbooks found', {
                page: { ...pageResult, items },
              });
            }

            const formatted = items
              .map((p, idx) => {
                const created = p.createdAt ? String(p.createdAt) : 'unknown';
                const steps = Array.isArray(p.workflow) ? p.workflow.length : 0;
                return `${idx + 1}. id:${p.id} goal:"${p.goal}" initial:"${p.initialCommand || ''}" steps:${steps} createdAt:${created}`;
              })
              .join('\n');

            return createMCPStructuredResponse(formatted, {
              page: { ...pageResult, items },
              formattedText: formatted,
            });
          }

          let items = [...inMemory];
          items = items.filter((p) => p.agentId === currentAssistantId);

          if (!items || items.length === 0) {
            return createMCPStructuredResponse(
              'No playbooks found (in-memory)',
              { playbooks: items },
            );
          }

          const formatted = items
            .map((p, idx) => {
              const created = p.createdAt ? String(p.createdAt) : 'unknown';
              const steps = Array.isArray(p.workflow) ? p.workflow.length : 0;
              return `${idx + 1}. id:${p.id} goal:"${p.goal}" initial:"${p.initialCommand || ''}" steps:${steps} createdAt:${created}`;
            })
            .join('\n');

          return createMCPStructuredResponse(formatted, {
            playbooks: items,
            formattedText: formatted,
          });
        }

        case 'delete_playbook': {
          const id = String(a.id);
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id);
            if (!existing)
              return createMCPTextResponse(`Playbook ${id} not found`);
            await dbService.playbooks.delete(id);
            const formatted = formatPlaybook(existing as PlaybookRecord);
            return createMCPStructuredResponse(
              `Playbook deleted: ${formatted}`,
              { success: true, id },
            );
          }
          const idx = inMemory.findIndex((p) => p.id === id);
          if (idx === -1)
            return createMCPTextResponse(`Playbook ${id} not found`);
          const removed = inMemory.splice(idx, 1)[0];
          const formattedRemoved = formatPlaybook(removed);
          return createMCPStructuredResponse(
            `Playbook deleted (in-memory): ${formattedRemoved}`,
            { success: true, id },
          );
        }
        case 'get_playbook': {
          const id = String(a.id);
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id as string);
            if (!existing)
              return createMCPTextResponse(`Playbook ${id} not found`);
            // Build detailed formatted text
            const rec = existing as PlaybookRecord;
            const lines: string[] = [];
            lines.push(`id: ${rec.id}`);
            lines.push(`agentId: ${rec.agentId}`);
            lines.push(`goal: ${rec.goal}`);
            lines.push(`initialCommand: ${rec.initialCommand || ''}`);
            lines.push(`createdAt: ${rec.createdAt ?? 'unknown'}`);
            lines.push(`updatedAt: ${rec.updatedAt ?? 'unknown'}`);
            lines.push(
              `steps: ${Array.isArray(rec.workflow) ? rec.workflow.length : 0}`,
            );
            if (Array.isArray(rec.workflow) && rec.workflow.length > 0) {
              lines.push('--- workflow details ---');
              rec.workflow.forEach((s, idx) => {
                lines.push(`${idx + 1}. stepId: ${s?.stepId ?? '<no stepId>'}`);
                lines.push(`   description: ${s?.description ?? ''}`);
                const toolName = s?.action?.toolName ?? '<no action>';
                const purpose = s?.action?.purpose ?? '<no purpose>';
                lines.push(`   action.toolName: ${toolName}`);
                lines.push(`   action.purpose: ${purpose}`);
                lines.push(
                  `   requiredData: ${(s?.requiredData || []).join(', ')}`,
                );
                lines.push(`   outputVariable: ${s?.outputVariable ?? ''}`);
              });
            }
            if (existing.successCriteria) {
              lines.push('--- successCriteria ---');
              lines.push(
                `description: ${existing.successCriteria.description}`,
              );
              if (existing.successCriteria.requiredArtifacts)
                lines.push(
                  `requiredArtifacts: ${existing.successCriteria.requiredArtifacts.join(', ')}`,
                );
            }

            const formatted = lines.join('\n');
            return createMCPStructuredResponse(formatted, {
              playbook: existing,
            });
          }

          const existing = inMemory.find((p) => p.id === String(a.id));
          if (!existing)
            return createMCPTextResponse(`Playbook ${String(a.id)} not found`);
          const lines: string[] = [];
          lines.push(`id: ${existing.id}`);
          lines.push(`agentId: ${existing.agentId}`);
          lines.push(`goal: ${existing.goal}`);
          lines.push(`initialCommand: ${existing.initialCommand || ''}`);
          lines.push(`createdAt: ${existing.createdAt ?? 'unknown'}`);
          lines.push(`updatedAt: ${existing.updatedAt ?? 'unknown'}`);
          lines.push(
            `steps: ${Array.isArray(existing.workflow) ? existing.workflow.length : 0}`,
          );
          if (
            Array.isArray(existing.workflow) &&
            existing.workflow.length > 0
          ) {
            lines.push('--- workflow details ---');
            existing.workflow.forEach((s, idx) => {
              lines.push(`${idx + 1}. stepId: ${s?.stepId ?? '<no stepId>'}`);
              lines.push(`   description: ${s?.description ?? ''}`);
              const toolName = s?.action?.toolName ?? '<no action>';
              const purpose = s?.action?.purpose ?? '<no purpose>';
              lines.push(`   action.toolName: ${toolName}`);
              lines.push(`   action.purpose: ${purpose}`);
              lines.push(
                `   requiredData: ${(s?.requiredData || []).join(', ')}`,
              );
              lines.push(`   outputVariable: ${s?.outputVariable ?? ''}`);
            });
          }
          if (existing.successCriteria) {
            lines.push('--- successCriteria ---');
            lines.push(`description: ${existing.successCriteria.description}`);
            if (existing.successCriteria.requiredArtifacts)
              lines.push(
                `requiredArtifacts: ${existing.successCriteria.requiredArtifacts.join(', ')}`,
              );
          }

          const formatted = lines.join('\n');
          return createMCPStructuredResponse(formatted, { playbook: existing });
        }
        case 'update_playbook': {
          const id = String(a.id);
          const patch = (a.playbook as Partial<Playbook>) || {};
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id as string);
            if (!existing)
              return createMCPTextResponse(`Playbook ${id} not found`);
            const updatedWorkflow = (patch.workflow as Playbook['workflow'])
              ? (patch.workflow as Playbook['workflow']).map((s, i) => ({
                  stepId: s?.stepId ?? `${id}-step-${i + 1}`,
                  description: s?.description ?? '',
                  action: s?.action ?? { toolName: '', purpose: '' },
                  requiredData: s?.requiredData ?? [],
                  outputVariable: s?.outputVariable ?? '',
                }))
              : existing.workflow;

            const updated: PlaybookRecord = {
              ...existing,
              ...patch,
              workflow: updatedWorkflow,
              updatedAt: new Date(),
            } as PlaybookRecord;
            await dbService.playbooks.upsert(updated);
            const saved = await dbService.playbooks.read(id);
            const formatted = saved
              ? formatPlaybook(saved as PlaybookRecord)
              : `Playbook ${id} updated`;
            return createMCPStructuredResponse(
              `Playbook updated: ${formatted}`,
              { success: true, playbook: saved },
            );
          }

          const existing = inMemory.find((p) => p.id === id);
          if (!existing)
            return createMCPTextResponse(`Playbook ${id} not found`);
          if (patch.workflow) {
            existing.workflow = (patch.workflow as Playbook['workflow']).map(
              (s, i) => ({
                stepId: s?.stepId ?? `${id}-step-${i + 1}`,
                description: s?.description ?? '',
                action: s?.action ?? { toolName: '', purpose: '' },
                requiredData: s?.requiredData ?? [],
                outputVariable: s?.outputVariable ?? '',
              }),
            );
          }
          Object.assign(existing, patch);
          existing.updatedAt = new Date().toISOString();
          const formattedExisting = formatPlaybook(existing);
          return createMCPStructuredResponse(
            `Playbook updated (in-memory): ${formattedExisting}`,
            { success: true, playbook: existing },
          );
        }

        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (err) {
      return createMCPTextResponse(String(err));
    }
  },
  async setContext(context: Record<string, unknown>): Promise<void> {
    const assistantId = context.assistantId as string;
    if (assistantId) {
      currentAssistantId = assistantId;
    }
  },
};

export default playbookStore;
