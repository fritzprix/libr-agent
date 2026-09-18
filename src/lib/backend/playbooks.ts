import { safeInvoke } from './core';
import type {
  Playbook,
  PlaybookDefaultTargetSession,
  PlaybookStep,
} from '@/types/playbook';
import type { Page } from '@/lib/db/types';
import {
  normalizePlaybookWorkflow,
  PlaybookDefaultTargetSessionSchema,
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
  workflow: unknown; // steps array (legacy envelope still accepted on read)
  successCriteria?: unknown;
  createdAt: number;
  updatedAt: number;
  isBookmarked: boolean;
  defaultTargetSession?: unknown;
}

type DeserializedPlaybook = Playbook & {
  id: string;
  createdAt: Date;
  updatedAt: Date;
};

function parseDefaultTargetSession(
  value: unknown,
): PlaybookDefaultTargetSession | undefined {
  if (value == null) {
    return undefined;
  }
  const result = PlaybookDefaultTargetSessionSchema.safeParse(value);
  if (result.success) {
    return result.data;
  }
  logger.warn('Ignoring invalid defaultTargetSession on playbook DTO');
  return undefined;
}

function extractWorkflowParts(workflow: unknown): {
  steps: PlaybookStep[];
  defaultTargetSession?: PlaybookDefaultTargetSession;
} {
  if (typeof workflow === 'string') {
    const parsed = safeParsePlaybookWorkflow(workflow);
    if (parsed) {
      return {
        steps: parsed.steps,
        defaultTargetSession: parsed.defaultTargetSession,
      };
    }
    logger.warn('Invalid workflow JSON in playbook');
    return { steps: [] };
  }

  if (workflow == null) {
    return { steps: [] };
  }

  const normalized = normalizePlaybookWorkflow(workflow);
  if (normalized) {
    return {
      steps: normalized.steps,
      defaultTargetSession: normalized.defaultTargetSession,
    };
  }

  // Legacy: already an array of steps without envelope validation
  if (Array.isArray(workflow)) {
    return { steps: workflow as PlaybookStep[] };
  }

  logger.warn('Unrecognized playbook workflow shape');
  return { steps: [] };
}

/**
 * Persist pin as a dedicated column value (not inside workflow JSON).
 * Invalid pin configs (mode pin without sessionId) are dropped.
 */
export function serializeDefaultTargetSessionForBackend(
  target: Playbook['defaultTargetSession'],
): PlaybookDefaultTargetSession | null {
  if (!target) {
    return null;
  }
  if (target.mode === 'pin') {
    const sessionId = target.sessionId?.trim();
    if (!sessionId) {
      return null;
    }
    return { mode: 'pin', sessionId };
  }
  return {
    mode: 'self',
    ...(target.sessionId?.trim()
      ? { sessionId: target.sessionId.trim() }
      : {}),
  };
}

function deserializePlaybook(dto: PlaybookDto): DeserializedPlaybook {
  const { steps: workflow, defaultTargetSession: envelopeTarget } =
    extractWorkflowParts(dto.workflow);

  const defaultTargetSession =
    parseDefaultTargetSession(dto.defaultTargetSession) ?? envelopeTarget;

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
    defaultTargetSession,
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
    defaultTargetSession: serializeDefaultTargetSessionForBackend(
      playbook.defaultTargetSession,
    ),
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
    defaultTargetSession: serializeDefaultTargetSessionForBackend(
      playbook.defaultTargetSession,
    ),
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
