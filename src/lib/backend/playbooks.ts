import { safeInvoke } from './core';
import type { Playbook } from '@/types/playbook';
import type { Page } from '@/lib/db/types';
import {
  safeParsePlaybookWorkflow,
  safeParseSuccessCriteria,
} from '@/lib/schemas/playbook';
import { getLogger } from '@/lib/logger';

const logger = getLogger('PlaybooksBackend');

/**
 * Backend DTO for Playbook
 */
interface PlaybookDto {
  id: string;
  assistantId: string;
  goal: string;
  initialCommand?: string;
  workflow: unknown; // JSON
  successCriteria?: unknown; // JSON
  createdAt: number;
  updatedAt: number;
  isBookmarked: boolean;
}

// Frontend Playbook type lacks ID/createdAt sometimes depending on where it's used?
// Check `types/playbook.ts`:
// export interface Playbook { id?: string; agentId: string; ... }

function deserializePlaybook(dto: PlaybookDto): Playbook & {
  id: string;
  createdAt: Date;
  updatedAt: Date;
} {
  // Parse workflow JSON string to PlaybookStep[] with validation
  let workflow: Playbook['workflow'] = [];
  if (typeof dto.workflow === 'string') {
    const parsed = safeParsePlaybookWorkflow(dto.workflow);
    if (parsed) {
      workflow = parsed.steps;
    } else {
      logger.warn('Invalid workflow JSON in playbook', { id: dto.id });
      workflow = [];
    }
  } else if (dto.workflow && typeof dto.workflow === 'object') {
    // Already parsed, try to validate structure
    const validated = safeParsePlaybookWorkflow(JSON.stringify(dto.workflow));
    workflow = validated?.steps || (dto.workflow as Playbook['workflow']);
  }

  // Parse successCriteria JSON string with validation
  let successCriteria: Playbook['successCriteria'] = { description: '' };
  if (typeof dto.successCriteria === 'string') {
    const parsed = safeParseSuccessCriteria(dto.successCriteria);
    if (parsed) {
      successCriteria = parsed;
    } else {
      logger.warn('Invalid successCriteria JSON in playbook', {
        id: dto.id,
      });
      successCriteria = { description: '' };
    }
  } else if (dto.successCriteria && typeof dto.successCriteria === 'object') {
    successCriteria = dto.successCriteria as Playbook['successCriteria'];
  }

  return {
    id: dto.id,
    agentId: dto.assistantId,
    goal: dto.goal,
    initialCommand: dto.initialCommand || '',
    workflow,
    successCriteria,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
    isBookmarked: dto.isBookmarked,
  };
}

export async function createPlaybook(playbook: Playbook): Promise<Playbook> {
  const dto = await safeInvoke<PlaybookDto>('create_playbook', {
    id: playbook.id || '',
    assistantId: playbook.agentId,
    goal: playbook.goal,
    initialCommand: playbook.initialCommand,
    workflow: playbook.workflow,
    successCriteria: playbook.successCriteria,
  });
  return deserializePlaybook(dto);
}

export async function updatePlaybook(playbook: Playbook): Promise<Playbook> {
  if (!playbook.id) throw new Error('Playbook ID required for update');

  const dto = await safeInvoke<PlaybookDto>('update_playbook', {
    id: playbook.id,
    assistantId: playbook.agentId,
    goal: playbook.goal,
    workflow: playbook.workflow,
    successCriteria: playbook.successCriteria,
  });
  return deserializePlaybook(dto);
}

export async function deletePlaybook(
  id: string,
  agentId: string,
): Promise<void> {
  await safeInvoke<void>('delete_playbook', { id, assistantId: agentId });
}

export interface ListPlaybooksOptions extends Record<string, unknown> {
  agentId?: string; // Optional for global listing
  sortBy?: 'created_at' | 'assistant';
  sortOrder?: 'asc' | 'desc';
  bookmarkFirst?: boolean;
}

export async function listPlaybooks(options: ListPlaybooksOptions): Promise<
  (Playbook & {
    id: string;
    createdAt: Date;
    updatedAt: Date;
  })[]
> {
  // If agentId is undefined, pass empty string for global listing
  const assistantId = options.agentId || '';
  const payload = { ...options, assistantId };
  delete payload.agentId;
  const dtos = await safeInvoke<PlaybookDto[]>('list_playbooks', payload);
  return dtos.map(deserializePlaybook);
}

export async function togglePlaybookBookmark(
  id: string,
  bookmarked: boolean,
  agentId: string,
): Promise<void> {
  await safeInvoke<void>('toggle_playbook_bookmark', {
    id,
    assistantId: agentId,
    bookmarked,
  });
}

export async function getPlaybook(
  id: string,
  agentId: string,
): Promise<
  (Playbook & { id: string; createdAt: Date; updatedAt: Date }) | undefined
> {
  const dto = await safeInvoke<PlaybookDto | null>('get_playbook', {
    id,
    assistantId: agentId,
  });
  return dto ? deserializePlaybook(dto) : undefined;
}

export async function upsertPlaybook(playbook: Playbook): Promise<void> {
  if (!playbook.id) {
    throw new Error('Playbook ID is required for upsert');
  }
  const exists = await getPlaybook(playbook.id, playbook.agentId);
  if (exists) {
    await updatePlaybook(playbook);
  } else {
    await createPlaybook(playbook);
  }
}

export async function getPlaybooksPage(
  agentId: string,
  page: number,
  pageSize: number,
): Promise<Page<Playbook & { id: string; createdAt: Date; updatedAt: Date }>> {
  const all = await listPlaybooks({ agentId });
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
