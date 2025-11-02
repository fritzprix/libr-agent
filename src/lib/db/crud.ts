import type {
  Assistant,
  Message,
  Session,
  MCPServerEntity,
} from '@/models/chat';
import type { CRUD, DatabaseObject, Page } from './types';
import { LocalDatabase } from './service';
import type { Playbook } from '@/types/playbook';
type PlaybookRecord = Playbook & {
  id: string;
  createdAt?: Date;
  updatedAt?: Date;
};

/**
 * Creates a pagination object.
 * This helper function constructs a `Page` object which is used
 * throughout the application for paginated data responses.
 *
 * @template T The type of items in the page.
 * @param items The array of items for the current page.
 * @param page The current page number.
 * @param pageSize The number of items per page. If -1, all items are returned on a single page.
 * @param totalItems The total number of items across all pages.
 * @returns A `Page` object containing the items and pagination metadata.
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

  const totalPages = Math.ceil(totalItems / pageSize);

  return {
    items,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
};

/**
 * CRUD operations for managing `Assistant` objects in the local database.
 * This object provides a standardized interface for creating, reading,
 * updating, deleting, and paginating assistants.
 */
export const assistantsCRUD: CRUD<Assistant> = {
  upsert: async (assistant: Assistant) => {
    const now = new Date();
    if (!assistant.createdAt) assistant.createdAt = now;
    assistant.updatedAt = now;
    await LocalDatabase.getInstance().assistants.put(assistant);
  },
  upsertMany: async (assistants: Assistant[]) => {
    const now = new Date();
    const updatedAssistants = assistants.map((assistant) => ({
      ...assistant,
      createdAt: assistant.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().assistants.bulkPut(updatedAssistants);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().assistants.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().assistants.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Assistant>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.assistants.count();

    if (pageSize === -1) {
      const items = await db.assistants.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.assistants
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().assistants.count();
  },
};

/**
 * CRUD operations for managing `MCPServerEntity` objects in the local database.
 * This object provides a standardized interface for creating, reading,
 * updating, deleting, and paginating MCP server configurations.
 */
export const mcpServersCRUD: CRUD<MCPServerEntity> = {
  upsert: async (server: MCPServerEntity) => {
    const db = LocalDatabase.getInstance();
    const now = new Date();

    // Check for unique name constraint (case-insensitive)
    const existing = await db.mcpServers
      .filter(
        (s) =>
          s.name.toLowerCase() === server.name.toLowerCase() &&
          s.id !== server.id,
      )
      .first();

    if (existing) {
      throw new Error(`MCP server with name "${server.name}" already exists`);
    }

    if (!server.createdAt) server.createdAt = now;
    server.updatedAt = now;
    await db.mcpServers.put(server);
  },
  upsertMany: async (servers: MCPServerEntity[]) => {
    const db = LocalDatabase.getInstance();
    const now = new Date();

    // Check for duplicate names within the batch
    const nameMap = new Map<string, number>();
    servers.forEach((server) => {
      const lowerName = server.name.toLowerCase();
      nameMap.set(lowerName, (nameMap.get(lowerName) || 0) + 1);
      if (nameMap.get(lowerName)! > 1) {
        throw new Error(`Duplicate server name in batch: "${server.name}"`);
      }
    });

    // Check for conflicts with existing servers
    const existingServers = await db.mcpServers.toArray();
    const serverIds = new Set(servers.map((s) => s.id));

    for (const existing of existingServers) {
      if (!serverIds.has(existing.id)) {
        const conflict = servers.find(
          (s) => s.name.toLowerCase() === existing.name.toLowerCase(),
        );
        if (conflict) {
          throw new Error(
            `MCP server with name "${conflict.name}" already exists`,
          );
        }
      }
    }

    const updatedServers = servers.map((server) => ({
      ...server,
      createdAt: server.createdAt || now,
      updatedAt: now,
    }));
    await db.mcpServers.bulkPut(updatedServers);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().mcpServers.get(id);
  },
  delete: async (id: string) => {
    const db = LocalDatabase.getInstance();

    // Check if any assistant references this server
    const assistants = await db.assistants.toArray();
    const referencingAssistants = assistants.filter((a) =>
      a.mcpServerIds?.includes(id),
    );

    if (referencingAssistants.length > 0) {
      const names = referencingAssistants.map((a) => a.name).join(', ');
      throw new Error(
        `Cannot delete MCP server: it is used by ${referencingAssistants.length} assistant(s): ${names}`,
      );
    }

    await db.mcpServers.delete(id);
  },
  getPage: async (
    page: number,
    pageSize: number,
  ): Promise<Page<MCPServerEntity>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.mcpServers.count();

    if (pageSize === -1) {
      const items = await db.mcpServers.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.mcpServers
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().mcpServers.count();
  },
};

/**
 * Generic CRUD operations for managing `DatabaseObject` instances in the local database.
 * This can be used to store any type of object that conforms to the `DatabaseObject` interface.
 *
 * @template T The type of the data stored within the `DatabaseObject`.
 */
export const objectsCRUD: CRUD<
  DatabaseObject<unknown>,
  DatabaseObject<unknown>
> = {
  upsert: async <T>(object: DatabaseObject<T>) => {
    const now = new Date();
    if (!object.createdAt) object.createdAt = now;
    object.updatedAt = now;
    await LocalDatabase.getInstance().objects.put(
      object as DatabaseObject<unknown>,
    );
  },
  upsertMany: async <T>(objects: DatabaseObject<T>[]) => {
    const now = new Date();
    const updatedObjects = objects.map((obj) => ({
      ...obj,
      createdAt: obj.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().objects.bulkPut(
      updatedObjects as DatabaseObject<unknown>[],
    );
  },
  read: async <T>(key: string): Promise<DatabaseObject<T> | undefined> => {
    return LocalDatabase.getInstance().objects.get(key) as Promise<
      DatabaseObject<T> | undefined
    >;
  },
  delete: async (key: string) => {
    await LocalDatabase.getInstance().objects.delete(key);
  },
  getPage: async <T>(
    page: number,
    pageSize: number,
  ): Promise<Page<DatabaseObject<T>>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.objects.count();

    if (pageSize === -1) {
      const items = (await db.objects
        .orderBy('createdAt')
        .toArray()) as DatabaseObject<T>[];
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = (await db.objects
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray()) as DatabaseObject<T>[];

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().objects.count();
  },
};

/**
 * CRUD operations for managing `Session` objects in the local database.
 * Provides methods for standard database interactions with sessions.
 */
export const sessionsCRUD: CRUD<Session> = {
  upsert: async (session: Session) => {
    const now = new Date();
    if (!session.createdAt) session.createdAt = now;
    session.updatedAt = now;
    await LocalDatabase.getInstance().sessions.put(session);
  },
  upsertMany: async (sessions: Session[]) => {
    const now = new Date();
    const updatedSessions = sessions.map((session) => ({
      ...session,
      createdAt: session.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().sessions.bulkPut(updatedSessions);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().sessions.get(id);
  },
  /**
   * Deletes a session and all its related data in a single transaction.
   * This includes all messages, file stores, file contents, and file chunks
   * associated with the specified session ID.
   *
   * @param id The ID of the session to delete.
   */
  delete: async (id: string) => {
    const db = LocalDatabase.getInstance();
    // File content is handled by the Rust backend; on the frontend we only
    // need to remove messages and session row.
    await db.transaction('rw', [db.sessions, db.messages], async () => {
      await db.messages.where('sessionId').equals(id).delete();
      await db.sessions.delete(id);
    });
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Session>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.sessions.count();

    if (pageSize === -1) {
      const items = await db.sessions.orderBy('updatedAt').reverse().toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.sessions
      .orderBy('updatedAt')
      .reverse()
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().sessions.count();
  },
};

/**
 * CRUD operations for managing `Message` objects in the local database.
 * This provides a standard interface for interacting with chat messages.
 */
export const messagesCRUD: CRUD<Message> = {
  upsert: async (message: Message) => {
    const now = new Date();
    if (!message.createdAt) message.createdAt = now;
    message.updatedAt = now;
    await LocalDatabase.getInstance().messages.put(message);
  },
  upsertMany: async (messages: Message[]) => {
    const now = new Date();
    const updatedMessages = messages.map((msg) => ({
      ...msg,
      createdAt: msg.createdAt || now,
      updatedAt: now,
    }));
    await LocalDatabase.getInstance().messages.bulkPut(updatedMessages);
  },
  read: async (id: string) => {
    return LocalDatabase.getInstance().messages.get(id);
  },
  delete: async (id: string) => {
    await LocalDatabase.getInstance().messages.delete(id);
  },
  getPage: async (page: number, pageSize: number): Promise<Page<Message>> => {
    const db = LocalDatabase.getInstance();
    const totalItems = await db.messages.count();

    if (pageSize === -1) {
      const items = await db.messages.orderBy('createdAt').toArray();
      return createPage(items, page, pageSize, totalItems);
    }

    const offset = (page - 1) * pageSize;
    const items = await db.messages
      .orderBy('createdAt')
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items, page, pageSize, totalItems);
  },
  count: async (): Promise<number> => {
    return LocalDatabase.getInstance().messages.count();
  },
};

/**
 * CRUD operations for managing `FileStore` objects in the local database.
 * A FileStore represents a collection of files, typically associated with a session.
 */
// FileStore / FileContent / FileChunk CRUD removed: backend (Rust) is authoritative

/** CRUD for persisted Task records stored in the LocalDatabase.tasks table. */
export const playbooksCRUD: CRUD<PlaybookRecord> & {
  getPageForAgent: (
    agentId: string,
    page: number,
    pageSize: number,
  ) => Promise<Page<PlaybookRecord>>;
} = {
  upsert: async (playbook) => {
    const now = new Date();
    const maybeId = playbook as unknown as { id?: unknown };
    const maybeCreated = playbook as unknown as { createdAt?: unknown };
    const record: PlaybookRecord = {
      ...playbook,
      id: typeof maybeId.id === 'string' ? maybeId.id : String(Date.now()),
      createdAt:
        maybeCreated.createdAt instanceof Date ? maybeCreated.createdAt : now,
      updatedAt: now,
    } as PlaybookRecord;
    await LocalDatabase.getInstance().table('playbooks').put(record);
  },
  upsertMany: async (playbooksArr) => {
    const now = new Date();
    const updated = playbooksArr.map((t) => {
      const mid = t as unknown as { id?: unknown };
      const mcreated = t as unknown as { createdAt?: unknown };
      return {
        ...t,
        id:
          typeof mid.id === 'string'
            ? mid.id
            : String(Date.now() + Math.random()),
        createdAt:
          mcreated.createdAt instanceof Date
            ? (mcreated.createdAt as Date)
            : now,
        updatedAt: now,
      } as PlaybookRecord;
    });
    await LocalDatabase.getInstance().table('playbooks').bulkPut(updated);
  },
  read: async (id) => {
    return LocalDatabase.getInstance().table('playbooks').get(id) as Promise<
      PlaybookRecord | undefined
    >;
  },
  delete: async (id) => {
    await LocalDatabase.getInstance().table('playbooks').delete(id);
  },
  getPage: async (page, pageSize) => {
    const db = LocalDatabase.getInstance();
    const table = db.table('playbooks');
    const totalItems = await table.count();
    if (pageSize === -1) {
      const items = await table.toArray();
      return createPage(items as PlaybookRecord[], page, pageSize, totalItems);
    }
    const offset = (page - 1) * pageSize;
    const items = await table.offset(offset).limit(pageSize).toArray();
    return createPage(items as PlaybookRecord[], page, pageSize, totalItems);
  },
  count: async () => {
    return LocalDatabase.getInstance().table('playbooks').count();
  },

  // New method: agentId-filtered pagination
  getPageForAgent: async (agentId, page, pageSize) => {
    const db = LocalDatabase.getInstance();
    const table = db.table('playbooks');

    // Count total items for this agent using indexed query
    const totalItems = await table.where('agentId').equals(agentId).count();

    if (pageSize === -1) {
      // Return all items for this agent
      const items = await table.where('agentId').equals(agentId).toArray();
      return createPage(items as PlaybookRecord[], page, pageSize, totalItems);
    }

    // Paginate with offset and limit
    const offset = (page - 1) * pageSize;
    const items = await table
      .where('agentId')
      .equals(agentId)
      .offset(offset)
      .limit(pageSize)
      .toArray();

    return createPage(items as PlaybookRecord[], page, pageSize, totalItems);
  },
};

/**
 * CRUD operations for managing `FileContent` objects in the local database.
 * A FileContent represents a single file's metadata within a FileStore.
 */
// FileContent CRUD removed

/**
 * CRUD operations for managing `FileChunk` objects in the local database.
 * FileChunks store the actual binary data of files in smaller pieces.
 */
// FileChunk CRUD removed
