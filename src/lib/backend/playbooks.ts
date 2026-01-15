import { safeInvoke } from './core';
import type { Playbook } from '@/types/playbook';
import type { Page } from '@/lib/db/types';

/**
 * Backend DTO for Playbook
 */
interface PlaybookDto {
  id: string;
  sessionId: string;
  goal: string;
  initialCommand?: string;
  workflow: unknown; // JSON
  successCriteria?: unknown; // JSON
  createdAt: number;
  updatedAt: number;
}

// Frontend Playbook type lacks ID/createdAt sometimes depending on where it's used?
// Check `types/playbook.ts`:
// export interface Playbook { id?: string; agentId: string; ... }
// We need to map agentId <=> sessionId

function deserializePlaybook(
  dto: PlaybookDto,
): Playbook & { id: string; createdAt: Date; updatedAt: Date } {
  // Parse workflow JSON string to PlaybookStep[]
  let workflow: Playbook['workflow'] = [];
  if (typeof dto.workflow === 'string') {
    try {
      workflow = JSON.parse(dto.workflow) as Playbook['workflow'];
    } catch {
      workflow = [];
    }
  } else if (dto.workflow && typeof dto.workflow === 'object') {
    workflow = dto.workflow as Playbook['workflow'];
  }

  // Parse successCriteria JSON string to proper object
  let successCriteria: Playbook['successCriteria'] = { description: '' };
  if (typeof dto.successCriteria === 'string') {
    try {
      successCriteria = JSON.parse(
        dto.successCriteria,
      ) as Playbook['successCriteria'];
    } catch {
      successCriteria = { description: '' };
    }
  } else if (dto.successCriteria && typeof dto.successCriteria === 'object') {
    successCriteria = dto.successCriteria as Playbook['successCriteria'];
  }

  return {
    id: dto.id,
    agentId: dto.sessionId, // Mapping sessionId to agentId
    goal: dto.goal,
    initialCommand: dto.initialCommand || '',
    workflow,
    successCriteria,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  };
}

function serializePlaybook(playbook: Playbook): {
  id: string;
  sessionId: string;
  goal: string;
  initialCommand: string;
  workflow: string;
  successCriteria: string | null;
} {
  return {
    id: playbook.id || '',
    sessionId: playbook.agentId,
    goal: playbook.goal,
    initialCommand: playbook.initialCommand,
    workflow: JSON.stringify(playbook.workflow),
    successCriteria: playbook.successCriteria
      ? JSON.stringify(playbook.successCriteria)
      : null,
  };
}

export async function createPlaybook(playbook: Playbook): Promise<Playbook> {
  const params = serializePlaybook(playbook);
  const dto = await safeInvoke<PlaybookDto>('create_playbook', {
    id: params.id,
    sessionId: params.sessionId,
    goal: params.goal,
    initialCommand: params.initialCommand,
    workflow: JSON.parse(params.workflow),
    successCriteria: params.successCriteria
      ? JSON.parse(params.successCriteria)
      : null,
  });
  return deserializePlaybook(dto);
}

export async function updatePlaybook(playbook: Playbook): Promise<Playbook> {
  const params = serializePlaybook(playbook);
  if (!params.id) throw new Error('Playbook ID required for update');

  const dto = await safeInvoke<PlaybookDto>('update_playbook', {
    id: params.id,
    sessionId: params.sessionId,
    goal: params.goal,
    workflow: JSON.parse(params.workflow),
    successCriteria: params.successCriteria
      ? JSON.parse(params.successCriteria)
      : null,
  });
  return deserializePlaybook(dto);
}

export async function deletePlaybook(id: string): Promise<void> {
  await safeInvoke<void>('delete_playbook', { id });
}

export async function listPlaybooks(): Promise<
  (Playbook & { id: string; createdAt: Date; updatedAt: Date })[]
> {
  const dtos = await safeInvoke<PlaybookDto[]>('list_playbooks');
  return dtos.map(deserializePlaybook);
}

export async function getPlaybook(
  id: string,
): Promise<
  (Playbook & { id: string; createdAt: Date; updatedAt: Date }) | undefined
> {
  // Backend doesn't have get_playbook command??
  // Checked command list: `create_playbook`, `update_playbook`, `delete_playbook`, `list_playbooks`.
  // No `get_playbook`.
  // Simulation:
  const all = await listPlaybooks();
  return all.find((p) => p.id === id);
}

export async function upsertPlaybook(playbook: Playbook): Promise<void> {
  if (!playbook.id) {
    // Treat as create? But we normally generate ID in frontend.
    throw new Error('Playbook ID is required for upsert');
  }
  const exists = await getPlaybook(playbook.id);
  if (exists) {
    await updatePlaybook(playbook);
  } else {
    await createPlaybook(playbook);
  }
}

export async function getPlaybooksPage(
  page: number,
  pageSize: number,
): Promise<Page<Playbook & { id: string; createdAt: Date; updatedAt: Date }>> {
  const all = await listPlaybooks();
  const totalItems = all.length;

  if (pageSize === -1) {
    return {
      items: all,
      page: 1,
      pageSize: totalItems,
      totalItems,
      totalPages: 1,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  const totalPages = Math.ceil(totalItems / pageSize) || 1;
  const start = (page - 1) * pageSize;
  const end = start + pageSize;
  const items = all.slice(start, end);

  return {
    items,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
}
