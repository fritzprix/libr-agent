import {
  createMCPStructuredResponse,
  createMCPTextResponse,
  createMCPStructuredMultipartResponse,
} from '@/lib/mcp-response-utils';
import type {
  MCPResponse,
  MCPTool,
  WebMCPServer,
  MCPContent,
} from '@/lib/mcp-types';
import type { Playbook } from '@/types/playbook';
import { dbService } from '@/lib/db';
import { createUIResource, type UIResource } from '@mcp-ui/server';
import type { ServiceInfo } from '@/lib/mcp-types';
import type { ServiceContextOptions } from '@/features/tools';

function escapeHtml(s: string): string {
  if (!s) return '';
  return String(s)
    .split('&')
    .join('&amp;')
    .split('<')
    .join('&lt;')
    .split('>')
    .join('&gt;')
    .split('"')
    .join('&quot;')
    .split("'")
    .join('&#39;');
}

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
      return `<div class="pb-item" style="padding:8px;border-bottom:1px solid #eee;display:flex;justify-content:space-between;align-items:center;"><div style="flex:1"><strong>${goal}</strong><div style="font-size:12px;color:#666">id:${id} • steps:${steps}</div></div><div><button data-pbid="${id}" class="select-pb-btn" style="margin-right:8px;">Select</button><button data-pbid="${id}" class="delete-pb-btn" style="background-color:#dc3545;color:white;border:none;padding:4px 8px;border-radius:4px;">Delete</button></div></div>`;
    })
    .join('');
}

function buildUiHtml(listItemsHtml: string, pageInfo: PageResult): string {
  const prevDisabled = pageInfo.page <= 1 ? 'disabled' : '';
  const nextDisabled = pageInfo.page >= pageInfo.totalPages ? 'disabled' : '';

  return `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <style>
    body {
      font-family: system-ui, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
      margin: 0;
      padding: 12px;
    }
    .pb-item {
      padding: 8px;
      border-bottom: 1px solid #eee;
      display: flex;
      justify-content: space-between;
      align-items: center;
    }
    .pagination {
      margin-top: 16px;
      display: flex;
      justify-content: center;
      align-items: center;
      gap: 8px;
      padding: 12px;
      border-top: 1px solid #eee;
    }
    button {
      padding: 6px 12px;
      border: 1px solid #ddd;
      border-radius: 4px;
      background: white;
      cursor: pointer;
      font-size: 14px;
    }
    button:disabled {
      opacity: 0.5;
      cursor: not-allowed;
    }
    button:hover:not(:disabled) {
      background: #f5f5f5;
    }
    .delete-pb-btn {
      background-color: #dc3545;
      color: white;
      border: none;
    }
    .delete-pb-btn:hover:not(:disabled) {
      background-color: #c82333;
    }
    .page-info {
      padding: 6px 12px;
      color: #666;
      font-size: 14px;
    }
  </style>
</head>
<body>
  <h3>Playbooks</h3>
  <div id="pb-list">${listItemsHtml}</div>

  <!-- Pagination UI -->
  <div class="pagination">
    <button data-page="${pageInfo.page - 1}" class="nav-page-btn" ${prevDisabled}>
      ← Previous
    </button>
    <span class="page-info">
      Page ${pageInfo.page} of ${pageInfo.totalPages} (${pageInfo.totalItems} total)
    </span>
    <button data-page="${pageInfo.page + 1}" class="nav-page-btn" ${nextDisabled}>
      Next →
    </button>
  </div>

  <script>
document.addEventListener('click', function(e) {
  const btn = e.target;
  if (btn && btn.classList) {
    const id = btn.getAttribute('data-pbid');
    const page = btn.getAttribute('data-page');

    if (btn.classList.contains('select-pb-btn')) {
      console.log('Select button clicked for id:', id);
      // Security note: Using '*' as targetOrigin for compatibility.
      // In production, consider restricting to specific origin if parent context is known.
      window.parent.postMessage({type:'tool', payload:{toolName:'select_playbook', params:{id}}}, '*');
    } else if (btn.classList.contains('delete-pb-btn')) {
      console.log('Delete button clicked for id:', id);
      window.parent.postMessage({type:'tool', payload:{toolName:'delete_playbook', params:{id}}}, '*');
    } else if (btn.classList.contains('nav-page-btn') && !btn.disabled) {
      console.log('Navigate to page:', page);
      window.parent.postMessage({type:'tool', payload:{toolName:'get_playbook_page', params:{page: parseInt(page)}}}, '*');
    }
  }
});
</script>
</body>
</html>`;
}

function createUiResourceFromHtml(html: string, toolName = 'show_playbooks') {
  const res = createUIResource({
    uri: `ui://playbooks/list/${Date.now()}`,
    content: { type: 'rawHtml', htmlString: html },
    encoding: 'text',
  }) as UIResource & { serviceInfo?: ServiceInfo };

  // Attach serviceInfo so the frontend can resolve tool names correctly
  // Use the canonical server name ('playbook') and mark this as a built-in web server
  res.serviceInfo = {
    serverName: 'playbook',
    toolName,
    backendType: 'BuiltInWeb',
  };

  return res;
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

// --- end helpers ---

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
