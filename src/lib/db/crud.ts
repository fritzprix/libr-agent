import type {
  Assistant,
  Message,
  Session,
  MCPServerEntity,
} from '@/models/chat';
import type { CRUD, DatabaseObject, Page } from './types';
import type { Playbook } from '@/types/playbook';

import * as assistantsBackend from '@/lib/backend/assistants';
import * as mcpBackent from '@/lib/backend/mcp-server-config';
import * as settingsBackend from '@/lib/backend/settings';
import * as sessionsBackend from '@/lib/backend/session-crud';
import * as messagesBackend from '@/lib/backend/messages';
import * as playbooksBackend from '@/lib/backend/playbooks';

/**
 * Validates pagination parameters and returns defaults if invalid
 */
function validatePagination(
  page: number,
  pageSize: number,
): { page: number; pageSize: number } {
  return {
    page: Math.max(1, page),
    pageSize: pageSize === -1 ? -1 : Math.max(1, pageSize),
  };
}

/**
 * Creates a pagination object. (Helper - typically backend handles paging or we simulate)
 * Keeping this if we need to simulate paging from backend lists.
 */
export const createPage = <T>(
  items: T[],
  page: number,
  pageSize: number,
  totalItems: number,
): Page<T> => {
  if (pageSize === -1) {
    return {
      items,
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
  const paginatedItems = items.slice(start, end);

  return {
    items: paginatedItems,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
};

/**
 * CRUD operations for managing `Assistant` objects via Rust Backend.
 */
export const assistantsCRUD: CRUD<Assistant> = {
  upsert: async (assistant: Assistant) => {
    await assistantsBackend.upsertAssistant(assistant);
  },
  upsertMany: async (assistants: Assistant[]) => {
    await assistantsBackend.upsertAssistants(assistants);
  },
  read: async (id: string) => {
    return assistantsBackend.getAssistant(id);
  },
  delete: async (id: string) => {
    await assistantsBackend.deleteAssistant(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Assistant>> => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    return assistantsBackend.getAssistantsPage(p, ps);
  },
  count: async (): Promise<number> => {
    const all = await assistantsBackend.listAssistants();
    return all.length;
  },
};

/**
 * CRUD operations for managing `MCPServerEntity` objects via Rust Backend.
 */
export const mcpServersCRUD: CRUD<MCPServerEntity> = {
  upsert: async (server: MCPServerEntity) => {
    // Unique check is handled by backend or upsert logic
    await mcpBackent.upsertMCPServer(server);
  },
  upsertMany: async (servers: MCPServerEntity[]) => {
    for (const s of servers) {
      await mcpBackent.upsertMCPServer(s);
    }
  },
  read: async (id: string) => {
    return mcpBackent.getMCPServer(id);
  },
  delete: async (id: string) => {
    // Check references
    const assistants = await assistantsBackend.listAssistants();
    const referencingAssistants = assistants.filter((a) =>
      a.mcpServerIds?.includes(id),
    );

    if (referencingAssistants.length > 0) {
      const names = referencingAssistants.map((a) => a.name).join(', ');
      throw new Error(
        `Cannot delete MCP server: it is used by ${referencingAssistants.length} assistant(s): ${names}`,
      );
    }

    await mcpBackent.deleteMCPServer(id);
  },
  getPage: async (
    page: number,
    pageSize: number,
  ): Promise<Page<MCPServerEntity>> => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    return mcpBackent.getMCPServersPage(p, ps);
  },
  count: async (): Promise<number> => {
    const all = await mcpBackent.listMCPServers();
    return all.length;
  },
};

/**
 * Generic CRUD operations for managing `DatabaseObject` instances via Rust Backend (Settings).
 */
export const objectsCRUD: CRUD<
  DatabaseObject<unknown>,
  DatabaseObject<unknown>
> = {
  upsert: async <T>(object: DatabaseObject<T>) => {
    await settingsBackend.upsertSetting(object);
  },
  upsertMany: async <T>(objects: DatabaseObject<T>[]) => {
    await settingsBackend.upsertSettings(objects);
  },
  read: async <T>(key: string): Promise<DatabaseObject<T> | undefined> => {
    return settingsBackend.getSetting<T>(key);
  },
  delete: async (key: string) => {
    await settingsBackend.deleteSetting(key);
  },
  getPage: async <T>(
    page: number,
    pageSize: number,
  ): Promise<Page<DatabaseObject<T>>> => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    return settingsBackend.getSettingsPage<T>(p, ps);
  },
  count: async (): Promise<number> => {
    const all = await settingsBackend.listSettings();
    return all.length;
  },
};

/**
 * CRUD operations for managing `Session` objects via Rust Backend.
 */
export const sessionsCRUD: CRUD<Session> = {
  upsert: async (session: Session) => {
    await sessionsBackend.upsertSession(session);
  },
  upsertMany: async (sessions: Session[]) => {
    for (const s of sessions) {
      await sessionsBackend.upsertSession(s);
    }
  },
  read: async (id: string) => {
    return sessionsBackend.getSession(id);
  },
  delete: async (id: string) => {
    await sessionsBackend.deleteSession(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Session>> => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    return sessionsBackend.getSessionsPage(p, ps);
  },
  count: async (): Promise<number> => {
    const all = await sessionsBackend.listSessions();
    return all.length;
  },
};

/**
 * CRUD operations for managing `Message` objects via Rust Backend.
 */
export const messagesCRUD: CRUD<Message> = {
  upsert: async (message: Message) => {
    await messagesBackend.upsertMessage(message);
  },
  upsertMany: async (messages: Message[]) => {
    await messagesBackend.upsertMessages(messages);
  },
  read: async (id: string) => {
    // Backend migration: Read single message not directly supported yet.
    // Simulating or returning undefined. logic flow usually doesn't need single message read by ID except internally.
    console.warn(
      'Single message read not supported in backend migration yet. returning undefined for id:',
      id,
    );
    return undefined;
  },
  delete: async (id: string) => {
    await messagesBackend.deleteMessage(id);
  },
  getPage: async (
    page: number,
    pageSize: number,
    filter?: { sessionId?: string; threadId?: string },
  ): Promise<Page<Message>> => {
    // If filter is provided, use specialized backend call
    if (filter?.sessionId && filter?.threadId) {
      const { page: p, pageSize: ps } = validatePagination(page, pageSize);
      return (await messagesBackend.getMessagesPageForSession(
        filter.sessionId,
        filter.threadId,
        p,
        ps,
      )) as Page<Message>;
    }
    // Fallback: empty page if no session filter (global message list not supported)
    return createPage([], page, pageSize, 0);
  },
  count: async (): Promise<number> => {
    return 0; // Not supported globally
  },
};

/**
 * CRUD operations for managing `FileStore` objects in the local database.
 * A FileStore represents a collection of files, typically associated with a session.
 */
// FileStore / FileContent / FileChunk CRUD removed: backend (Rust) is authoritative
type PlaybookRecord = Playbook & {
  id: string;
  createdAt?: Date;
  updatedAt?: Date;
};

/** CRUD for persisted Task records stored in the Backend. */
export const playbooksCRUD: CRUD<PlaybookRecord> & {
  getPageForAgent: (
    agentId: string,
    page: number,
    pageSize: number,
  ) => Promise<Page<PlaybookRecord>>;
} = {
  upsert: async (playbook: PlaybookRecord) => {
    // PlaybookRecord includes id and timestamps which backend handles
    await playbooksBackend.upsertPlaybook(playbook);
  },
  upsertMany: async (playbooksArr: PlaybookRecord[]) => {
    for (const p of playbooksArr) {
      await playbooksBackend.upsertPlaybook(p);
    }
  },
  read: async (id: string, agentId?: string) => {
    if (!agentId) throw new Error('agentId required to read playbook');
    return playbooksBackend.getPlaybook(id, agentId);
  },
  delete: async (id: string, agentId?: string) => {
    if (!agentId) throw new Error('agentId required to delete playbook');
    await playbooksBackend.deletePlaybook(id, agentId);
  },
  getPage: async (page: number, pageSize: number, agentId?: string) => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    if (!agentId) throw new Error('agentId required to page playbooks');
    return playbooksBackend.getPlaybooksPage(agentId, p, ps);
  },
  count: async (agentId?: string) => {
    if (!agentId) return 0;
    const all = await playbooksBackend.listPlaybooks({ agentId });
    return all.length;
  },

  // New method: agentId-filtered pagination
  getPageForAgent: async (agentId: string, page: number, pageSize: number) => {
    const { page: p, pageSize: ps } = validatePagination(page, pageSize);
    return playbooksBackend.getPlaybooksPage(agentId, p, ps);
  },
};
