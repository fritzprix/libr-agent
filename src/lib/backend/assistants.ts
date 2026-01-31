import { safeInvoke } from './core';
import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { parseAssistant } from '@/models/validation';

/**
 * Backend DTO for Assistant
 */
interface AssistantDto {
  id: string;
  name: string;
  config: unknown; // JSON
  createdAt: number;
  updatedAt: number;
}

// Convert frontend Assistant model to backend params
function serializeAssistant(assistant: Assistant): {
  id: string;
  name: string;
  config: string;
} {
  // Extract top-level fields that are stored separately in backend
  // createdAt and updatedAt are not included in serialization as backend manages timestamps
  const { id, name, ...configRest } = assistant;

  // Everything else goes into config JSON
  return {
    id: id || '',
    name: name,
    config: JSON.stringify(configRest),
  };
}

/**
 * Creates a new assistant on the backend.
 */
export async function createAssistant(
  assistant: Assistant,
): Promise<Assistant> {
  const params = serializeAssistant(assistant);
  const dto = await safeInvoke<AssistantDto>('create_assistant', {
    id: params.id,
    name: params.name,
    config: JSON.parse(params.config), // Rust generic Value usually expects parsed JSON or string? Command expectation: Value which implies parsed JSON in invoke args
  });
  return parseAssistant(dto);
}

/**
 * Updates an existing assistant on the backend.
 */
export async function updateAssistant(
  assistant: Assistant,
): Promise<Assistant> {
  const params = serializeAssistant(assistant);
  const dto = await safeInvoke<AssistantDto>('update_assistant', {
    id: params.id,
    name: params.name,
    config: JSON.parse(params.config),
  });
  return parseAssistant(dto);
}

/**
 * Gets a single assistant by ID.
 */
export async function getAssistant(id: string): Promise<Assistant | undefined> {
  const dto = await safeInvoke<AssistantDto | null>('get_assistant', { id });
  return dto ? parseAssistant(dto) : undefined;
}

/**
 * Deletes an assistant by ID.
 */
export async function deleteAssistant(id: string): Promise<void> {
  await safeInvoke<void>('delete_assistant', { id });
}

/**
 * Lists all assistants.
 */
export async function listAssistants(): Promise<Assistant[]> {
  const dtos = await safeInvoke<AssistantDto[]>('list_assistants');
  return dtos.map(parseAssistant);
}

/**
 * Upsert implementation (Check existence then Create/Update)
 * Since SeaORM doesn't have a direct upsert that returns the model easily without conflict handling,
 * we verify existence first to match Dexie's behavior.
 * NOTE: For high concurrency this might be race-prone, but fine for local app.
 */
export async function upsertAssistant(assistant: Assistant): Promise<void> {
  if (!assistant.id) {
    throw new Error('Assistant ID is required for upsert');
  }
  const exists = await getAssistant(assistant.id);
  if (exists) {
    await updateAssistant(assistant);
  } else {
    await createAssistant(assistant);
  }
}

/**
 * Upserts multiple assistants
 */
export async function upsertAssistants(assistants: Assistant[]): Promise<void> {
  // Serial execution for now to ensure correctness
  for (const a of assistants) {
    await upsertAssistant(a);
  }
}

/**
 * Gets a page of assistants (simulation via listAssistants)
 */
export async function getAssistantsPage(
  page: number,
  pageSize: number,
): Promise<Page<Assistant>> {
  const all = await listAssistants();
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
