import {
  createMCPStructuredResponse,
  createMCPTextResponse,
  createMCPStructuredMultipartResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, WebMCPServer, MCPContent } from '@/lib/mcp-types';
import type { Playbook } from '@/types/playbook';
import { dbService } from '@/lib/db';
import type { ServiceContextOptions } from '@/features/tools';

import {
  escapeHtml,
  createUiResourceWithServiceInfo,
  renderTemplate,
} from '../../utils/ui-utils';
import playbooksTemplate from './templates/playbooks.hbs?raw';
import { playbookTools as tools } from './tools.ts';

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

// --- Helpers to reduce duplication between DB and in-memory branches ---
function formatPlaybooksList(items: PlaybookRecord[]): string {
  return items
    .map((p, idx) => {
      const created = p.createdAt ? String(p.createdAt) : 'unknown';
      const steps = Array.isArray(p.workflow) ? p.workflow.length : 0;
      return `${idx + 1}. id:${p.id} goal:"${p.goal}" initial:"${p.initialCommand || ''}" steps:${steps} createdAt:${created}`;
    })
    .join('\n');
}

function buildListItemsHtml(items: PlaybookRecord[]): string {
  return items
    .map((p) => {
      const goal = escapeHtml(p.goal);
      const id = escapeHtml(p.id);
      const steps = (p.workflow || []).length;
      return `<div class="pb-item" style="padding:8px;border-bottom:1px solid #eee;display:flex;justify-content:space-between;align-items:center;"><div style="flex:1"><strong>${goal}</strong><div style="font-size:12px;color:#666">id:${id} • steps:${steps}</div></div><div><button data-pbid="${id}" class="select-pb-btn" style="margin-right:8px;">Select</button><button data-pbid="${id}" class="delete-pb-btn" style="\n      background-color:#dc3545;color:white;border:none;padding:4px 8px;border-radius:4px;\n    ">Delete</button></div></div>`;
    })
    .join('');
}

function buildUiHtml(listItemsHtml: string, pageInfo: PageResult): string {
  const prevDisabled = pageInfo.page <= 1 ? 'disabled' : '';
  const nextDisabled = pageInfo.page >= pageInfo.totalPages ? 'disabled' : '';

  return renderTemplate(playbooksTemplate, {
    listItemsHtml,
    prevPage: pageInfo.page - 1,
    nextPage: pageInfo.page + 1,
    prevDisabled,
    nextDisabled,
    page: pageInfo.page,
    totalPages: pageInfo.totalPages,
    totalItems: pageInfo.totalItems,
  });
}

function createUiResourceFromHtml(html: string, toolName = 'show_playbooks') {
  // Use shared helper to construct a UIResource with consistent serviceInfo.
  return createUiResourceWithServiceInfo({
    html,
    serverName: 'playbook',
    toolName,
    uri: `ui://playbooks/list/${Date.now()}`,
  });
}

function makeListMultipartResponse(
  items: PlaybookRecord[],
  formattedText: string,
  structured: unknown,
  pageInfo: PageResult,
  toolName = 'show_playbooks',
) {
  const textPart: MCPContent = {
    type: 'text',
    text: formattedText,
  } as unknown as MCPContent;
  const uiHtml = buildUiHtml(buildListItemsHtml(items), pageInfo);
  const uiResource = {
    ...createUiResourceFromHtml(uiHtml, toolName),
  };
  return createMCPStructuredMultipartResponse(
    [textPart, uiResource],
    structured,
  );
}

function formatPlaybookDetailed(rec: PlaybookRecord) {
  const lines: string[] = [];
  lines.push(`id: ${rec.id}`);
  lines.push(`agentId: ${rec.agentId}`);
  lines.push(`goal: ${rec.goal}`);
  lines.push(`initialCommand: ${rec.initialCommand || ''}`);
  lines.push(`createdAt: ${rec.createdAt ?? 'unknown'}`);
  lines.push(`updatedAt: ${rec.updatedAt ?? 'unknown'}`);
  lines.push(`steps: ${Array.isArray(rec.workflow) ? rec.workflow.length : 0}`);
  if (Array.isArray(rec.workflow) && rec.workflow.length > 0) {
    lines.push('--- workflow details ---');
    rec.workflow.forEach((s, idx) => {
      lines.push(`${idx + 1}. stepId: ${s?.stepId ?? '<no stepId>'}`);
      lines.push(`   description: ${s?.description ?? ''}`);
      const toolName = s?.action?.toolName ?? '<no action>';
      const purpose = s?.action?.purpose ?? '<no purpose>';
      lines.push(`   action.toolName: ${toolName}`);
      lines.push(`   action.purpose: ${purpose}`);
      lines.push(`   requiredData: ${(s?.requiredData || []).join(', ')}`);
      lines.push(`   outputVariable: ${s?.outputVariable ?? ''}`);
    });
  }
  if (rec.successCriteria) {
    lines.push('--- successCriteria ---');
    lines.push(`description: ${rec.successCriteria.description}`);
    if (rec.successCriteria.requiredArtifacts)
      lines.push(
        `requiredArtifacts: ${rec.successCriteria.requiredArtifacts.join(', ')}`,
      );
  }
  return lines.join('\n');
}

// --- Phase 1 Helper Functions ---

interface PageResult {
  page: number;
  pageSize: number;
  totalItems: number;
  totalPages: number;
  items: PlaybookRecord[];
}

/**
 * Apply pagination to filtered items (in-memory mode)
 */
function paginateItems(
  items: PlaybookRecord[],
  page: number,
  pageSize: number,
): PageResult {
  const totalItems = items.length;

  if (pageSize === -1) {
    // Return all items
    return {
      page: 1,
      pageSize: -1,
      totalItems,
      totalPages: 1,
      items,
    };
  }

  const totalPages = Math.ceil(totalItems / pageSize);
  const startIdx = (page - 1) * pageSize;
  const endIdx = startIdx + pageSize;
  const paginatedItems = items.slice(startIdx, endIdx);

  return {
    page,
    pageSize,
    totalItems,
    totalPages,
    items: paginatedItems,
  };
}

/**
 * Create consistent structured response for list operations
 */
function createListStructuredResponse(
  pageResult: PageResult,
  formattedText: string,
): unknown {
  return {
    page: pageResult,
    formattedText,
  };
}

/**
 * Create context-aware text response for list_playbooks
 */
function createListTextResponse(
  agentId: string,
  pageResult: PageResult,
  formattedList: string,
): string {
  if (pageResult.totalItems === 0) {
    return `[list_playbooks] No playbooks found for agent ${agentId}.`;
  }

  return `[list_playbooks] Found ${pageResult.totalItems} playbook(s) for agent ${agentId}.
Showing page ${pageResult.page} of ${pageResult.totalPages} (${pageResult.items.length} items on this page):

${formattedList}

Note: Use 'get_playbook' to view details or 'select_playbook' to execute a playbook.`;
}

/**
 * Create context-aware text response for UI-based tools (show_playbooks, get_playbook_page)
 */
function createUITextResponse(
  toolName: string,
  pageResult: PageResult,
  formattedList: string,
  isNavigation = false,
): string {
  const action = isNavigation
    ? `Navigated to page ${pageResult.page}`
    : `Displaying ${pageResult.totalItems} playbook(s) in interactive UI`;

  return `[${toolName}] ${action}.
Current page: ${pageResult.page} of ${pageResult.totalPages}

Playbooks on this page:
${formattedList}

Status: Agent paused for user interaction (Select/Delete/Navigate buttons available).`;
}

/**
 * Common logic for UI-based playbook listing (show_playbooks, get_playbook_page)
 */
async function getPlaybooksWithUI(
  agentId: string,
  page: number,
  pageSize: number,
  hasDB: boolean,
  toolName: string,
  isNavigation = false,
): Promise<MCPResponse<unknown>> {
  if (hasDB && dbService.playbooks) {
    // DB branch: use indexed query
    const dbPageResult = await dbService.playbooks.getPageForAgent(
      agentId,
      page,
      pageSize,
    );

    // Convert Page<T> to PageResult
    const pageResult: PageResult = {
      page: dbPageResult.page,
      pageSize: dbPageResult.pageSize,
      totalItems: dbPageResult.totalItems,
      totalPages: dbPageResult.totalPages,
      items: dbPageResult.items as PlaybookRecord[],
    };

    if (pageResult.items.length === 0) {
      const emptyText = `[${toolName}] No playbooks found for agent ${agentId}.`;
      return createMCPStructuredResponse(emptyText, {
        page: pageResult,
      });
    }

    const formattedList = formatPlaybooksList(pageResult.items);
    const textResponse = createUITextResponse(
      toolName,
      pageResult,
      formattedList,
      isNavigation,
    );
    const structured = createListStructuredResponse(pageResult, formattedList);
    return makeListMultipartResponse(
      pageResult.items,
      textResponse,
      structured,
      pageResult,
      toolName,
    );
  }

  // In-memory branch
  const filtered = inMemory.filter((p) => p.agentId === agentId);
  const pageResult = paginateItems(filtered, page, pageSize);

  if (pageResult.items.length === 0) {
    const emptyText = `[${toolName}] No playbooks found (in-memory) for agent ${agentId}.`;
    return createMCPStructuredResponse(emptyText, {
      page: pageResult,
    });
  }

  const formattedList = formatPlaybooksList(pageResult.items);
  const textResponse = createUITextResponse(
    toolName,
    pageResult,
    formattedList,
    isNavigation,
  );
  const structured = createListStructuredResponse(pageResult, formattedList);
  return makeListMultipartResponse(
    pageResult.items,
    textResponse,
    structured,
    pageResult,
    toolName,
  );
}

const playbookStore: WebMCPServer = {
  name: 'playbook',
  version: '0.1.1',
  description:
    'Persisted playbook store for agent workflows (create/list/show/navigate/update/delete)',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    const a = (args || {}) as Record<string, unknown>;

    const hasDB = !!dbService.playbooks;

    try {
      switch (name) {
        case 'create_playbook': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              '[create_playbook] Error: Assistant ID not set. Please set context first.',
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

            const textResponse = `[create_playbook] Successfully created new playbook.
ID: ${id}
Goal: ${playbook.goal}
Steps: ${playbook.workflow.length}

${formatted}

The playbook is now available. Use 'list_playbooks' to see all playbooks, or 'select_playbook' with ID ${id} to execute it.`;

            return createMCPStructuredResponse(textResponse, {
              success: true,
              playbook: saved,
            });
          }

          inMemory.push(playbook);
          const formattedInMemory = formatPlaybook(playbook);

          const textResponse = `[create_playbook] Successfully created new playbook (in-memory).
ID: ${id}
Goal: ${playbook.goal}
Steps: ${playbook.workflow.length}

${formattedInMemory}

The playbook is now available. Use 'list_playbooks' to see all playbooks, or 'select_playbook' with ID ${id} to execute it.`;

          return createMCPStructuredResponse(textResponse, {
            success: true,
            playbook,
          });
        }

        case 'list_playbooks': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              '[list_playbooks] Error: Assistant ID not set. Please set context first.',
            );
          }

          const page = Number(a.page || 1);
          const pageSize = Number(a.pageSize || -1);

          if (hasDB && dbService.playbooks) {
            // DB: Use indexed query for filtered pagination
            const dbPageResult = await dbService.playbooks.getPageForAgent(
              currentAssistantId,
              page,
              pageSize,
            );

            // Convert Page<T> to PageResult
            const pageResult: PageResult = {
              page: dbPageResult.page,
              pageSize: dbPageResult.pageSize,
              totalItems: dbPageResult.totalItems,
              totalPages: dbPageResult.totalPages,
              items: dbPageResult.items as PlaybookRecord[],
            };

            const formattedList = formatPlaybooksList(pageResult.items);
            const textResponse = createListTextResponse(
              currentAssistantId,
              pageResult,
              formattedList,
            );
            const structured = createListStructuredResponse(
              pageResult,
              formattedList,
            );
            return createMCPStructuredResponse(textResponse, structured);
          }

          // In-memory branch
          const filtered = inMemory.filter(
            (p) => p.agentId === currentAssistantId,
          );
          const pageResult = paginateItems(filtered, page, pageSize);

          const formattedList = formatPlaybooksList(pageResult.items);
          const textResponse = createListTextResponse(
            currentAssistantId,
            pageResult,
            formattedList,
          );
          const structured = createListStructuredResponse(
            pageResult,
            formattedList,
          );
          return createMCPStructuredResponse(textResponse, structured);
        }

        case 'show_playbooks': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              '[show_playbooks] Error: Assistant ID not set. Please set context first.',
            );
          }

          const page = Number(a.page || 1);
          const pageSize = Number(a.pageSize || -1);

          return getPlaybooksWithUI(
            currentAssistantId,
            page,
            pageSize,
            hasDB,
            'show_playbooks',
            false,
          );
        }

        case 'get_playbook_page': {
          if (!currentAssistantId) {
            return createMCPTextResponse(
              '[get_playbook_page] Error: Assistant ID not set. Please set context first.',
            );
          }

          const page = Number(a.page || 1);
          const pageSize = Number(a.pageSize || 10); // Default to 10 if not specified

          return getPlaybooksWithUI(
            currentAssistantId,
            page,
            pageSize,
            hasDB,
            'get_playbook_page',
            true,
          );
        }

        case 'delete_playbook': {
          const id = String(a.id);
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id);
            if (!existing)
              return createMCPTextResponse(
                `[delete_playbook] Error: Playbook ${id} not found.`,
              );
            await dbService.playbooks.delete(id);
            const formatted = formatPlaybook(existing as PlaybookRecord);

            const textResponse = `[delete_playbook] Successfully deleted playbook ID: ${id}

Deleted Playbook:
${formatted}

This playbook is no longer available. Use 'list_playbooks' to see remaining playbooks.`;

            return createMCPStructuredResponse(textResponse, {
              success: true,
              id,
            });
          }
          const idx = inMemory.findIndex((p) => p.id === id);
          if (idx === -1)
            return createMCPTextResponse(
              `[delete_playbook] Error: Playbook ${id} not found (in-memory).`,
            );
          const removed = inMemory.splice(idx, 1)[0];
          const formattedRemoved = formatPlaybook(removed);

          const textResponse = `[delete_playbook] Successfully deleted playbook ID: ${id} (in-memory)

Deleted Playbook:
${formattedRemoved}

This playbook is no longer available. Use 'list_playbooks' to see remaining playbooks.`;

          return createMCPStructuredResponse(textResponse, {
            success: true,
            id,
          });
        }
        case 'get_playbook': {
          const id = String(a.id);
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id as string);
            if (!existing)
              return createMCPTextResponse(
                `[get_playbook] Error: Playbook ${id} not found.`,
              );
            const rec = existing as PlaybookRecord;
            const formatted = formatPlaybookDetailed(rec);

            const textResponse = `[get_playbook] Retrieved playbook details for ID: ${id}

${formatted}

Note: Use 'select_playbook' to execute this playbook, or 'update_playbook' to modify it.`;

            return createMCPStructuredResponse(textResponse, {
              playbook: existing,
            });
          }

          const existing = inMemory.find((p) => p.id === String(a.id));
          if (!existing)
            return createMCPTextResponse(
              `[get_playbook] Error: Playbook ${String(a.id)} not found (in-memory).`,
            );
          const formatted = formatPlaybookDetailed(existing);

          const textResponse = `[get_playbook] Retrieved playbook details for ID: ${String(a.id)} (in-memory)

${formatted}

Note: Use 'select_playbook' to execute this playbook, or 'update_playbook' to modify it.`;

          return createMCPStructuredResponse(textResponse, {
            playbook: existing,
          });
        }
        case 'update_playbook': {
          const id = String(a.id);
          const patch = (a.playbook as Partial<Playbook>) || {};
          if (hasDB && dbService.playbooks) {
            const existing = await dbService.playbooks.read(id as string);
            if (!existing)
              return createMCPTextResponse(
                `[update_playbook] Error: Playbook ${id} not found.`,
              );
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

            const textResponse = `[update_playbook] Successfully updated playbook ID: ${id}

Updated Details:
${formatted}

The playbook has been modified. Changes are immediately available.`;

            return createMCPStructuredResponse(textResponse, {
              success: true,
              playbook: saved,
            });
          }

          const existing = inMemory.find((p) => p.id === id);
          if (!existing)
            return createMCPTextResponse(
              `[update_playbook] Error: Playbook ${id} not found (in-memory).`,
            );
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

          const textResponse = `[update_playbook] Successfully updated playbook ID: ${id} (in-memory)

Updated Details:
${formattedExisting}

The playbook has been modified. Changes are immediately available.`;

          return createMCPStructuredResponse(textResponse, {
            success: true,
            playbook: existing,
          });
        }

        case 'select_playbook': {
          const id = String(a.id);
          let existing: PlaybookRecord | undefined;
          if (hasDB && dbService.playbooks) {
            existing = (await dbService.playbooks.read(id)) as
              | PlaybookRecord
              | undefined;
          } else {
            existing = inMemory.find((p) => p.id === id);
          }

          if (!existing) {
            return createMCPTextResponse(
              `[select_playbook] Error: Playbook ${id} not found.`,
            );
          }

          // Permission check
          if (currentAssistantId && existing.agentId !== currentAssistantId) {
            return createMCPTextResponse(
              `[select_playbook] Error: Playbook ${id} does not belong to the current assistant (${currentAssistantId}).`,
            );
          }

          // Build detailed text
          const formattedText = formatPlaybookDetailed(existing);

          const agentPrompt = `[select_playbook] Playbook "${existing.goal}" (ID: ${existing.id}) has been selected for execution.

Playbook Details:
---
${formattedText}
---

Instructions:
1. Review the workflow steps and success criteria above
2. Establish todos based on the workflow steps
3. Begin executing the tasks according to the defined steps
4. Track progress and verify against success criteria

You may now proceed with execution.`;

          return createMCPStructuredResponse(agentPrompt, {
            playbook: existing,
          });
        }

        default:
          return createMCPTextResponse(`Unknown tool: ${name}`);
      }
    } catch (err) {
      return createMCPTextResponse(String(err));
    }
  },
  async switchContext(context: ServiceContextOptions): Promise<void> {
    const assistantId = context.assistantId;
    if (assistantId) {
      currentAssistantId = assistantId;
    }
  },
};

export default playbookStore;
