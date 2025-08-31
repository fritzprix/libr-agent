import type { Assistant, Group, Message, Session } from '@/models/chat';
import Dexie, { Table } from 'dexie';
import type {
  DatabaseObject,
  DatabaseService,
  FileChunk,
  FileContent,
  FileStore,
  Page,
} from './types';
import {
  assistantsCRUD,
  fileChunksCRUD,
  fileContentsCRUD,
  fileStoresCRUD,
  groupsCRUD,
  messagesCRUD,
  objectsCRUD,
  sessionsCRUD,
} from './crud';

export class LocalDatabase extends Dexie {
  private static instance: LocalDatabase;
  public static getInstance(): LocalDatabase {
    if (!LocalDatabase.instance) {
      LocalDatabase.instance = new LocalDatabase();
    }
    return LocalDatabase.instance;
  }

  assistants!: Table<Assistant, string>;
  objects!: Table<DatabaseObject<unknown>, string>;
  sessions!: Table<Session, string>;
  messages!: Table<Message, string>;
  groups!: Table<Group, string>;
  fileStores!: Table<FileStore, string>;
  fileContents!: Table<FileContent, string>;
  fileChunks!: Table<FileChunk, string>;

  constructor() {
    super('MCPAgentDB');

    this.version(1).stores({
      assistants: '&id',
      objects: '&key',
    });

    this.version(2)
      .stores({
        assistants: '&id, createdAt, updatedAt, name',
        objects: '&key, createdAt, updatedAt',
      })
      .upgrade(async (tx) => {
        const now = new Date();

        await tx
          .table('assistants')
          .toCollection()
          .modify((assistant) => {
            if (!assistant.createdAt) assistant.createdAt = now;
            if (!assistant.updatedAt) assistant.updatedAt = now;
          });

        await tx
          .table('objects')
          .toCollection()
          .modify((obj) => {
            if (!obj.createdAt) obj.createdAt = now;
            if (!obj.updatedAt) obj.updatedAt = now;
          });
      });

    this.version(3).stores({
      sessions: '&id, createdAt, updatedAt',
      messages: '&id, sessionId, createdAt',
    });

    this.version(4).stores({
      groups: '&id, createdAt, updatedAt, name',
    });

    this.version(5).stores({
      fileStores: '&id, sessionId, createdAt, updatedAt, name',
      fileContents: '&id, storeId, filename, uploadedAt, mimeType',
      fileChunks: '&id, contentId, chunkIndex',
    });

    this.version(6).stores({
      fileStores: '&id, sessionId, createdAt, updatedAt, name',
      fileContents:
        '&id, storeId, filename, uploadedAt, mimeType, contentHash, [storeId+contentHash]',
      fileChunks: '&id, contentId, chunkIndex',
    });
  }
}

const createPage = <T>(
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
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  return {
    items,
    page,
    pageSize,
    totalItems,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
};

export const dbService: DatabaseService = {
  assistants: assistantsCRUD,
  objects: objectsCRUD,
  sessions: sessionsCRUD,
  messages: messagesCRUD,
  groups: groupsCRUD,
  fileStores: fileStoresCRUD,
  fileContents: fileContentsCRUD,
  fileChunks: fileChunksCRUD,
};

export const dbUtils = {
  // --- Assistants ---
  getAllAssistants: async (): Promise<Assistant[]> => {
    return LocalDatabase.getInstance()
      .assistants.orderBy('createdAt')
      .toArray();
  },
  assistantExists: async (id: string): Promise<boolean> => {
    return (await LocalDatabase.getInstance().assistants.get(id)) !== undefined;
  },
  clearAllAssistants: async (): Promise<void> => {
    await LocalDatabase.getInstance().assistants.clear();
  },
  bulkUpsertAssistants: async (assistants: Assistant[]): Promise<void> => {
    await dbService.assistants.upsertMany(assistants);
  },

  // --- Objects ---
  getAllObjects: async (): Promise<DatabaseObject<unknown>[]> => {
    return LocalDatabase.getInstance().objects.orderBy('createdAt').toArray();
  },
  objectExists: async (key: string): Promise<boolean> => {
    return (await LocalDatabase.getInstance().objects.get(key)) !== undefined;
  },
  clearAllObjects: async (): Promise<void> => {
    await LocalDatabase.getInstance().objects.clear();
  },
  bulkUpsertObjects: async (objects: DatabaseObject[]): Promise<void> => {
    await dbService.objects.upsertMany(objects);
  },

  // --- Sessions ---
  getAllSessions: async (): Promise<Session[]> => {
    return LocalDatabase.getInstance()
      .sessions.orderBy('updatedAt')
      .reverse()
      .toArray();
  },
  clearAllSessions: async (): Promise<void> => {
    await LocalDatabase.getInstance().sessions.clear();
    await LocalDatabase.getInstance().messages.clear();
  },
  bulkUpsertSessions: async (sessions: Session[]): Promise<void> => {
    await dbService.sessions.upsertMany(sessions);
  },

  // --- Messages ---
  getAllMessages: async (): Promise<Message[]> => {
    return LocalDatabase.getInstance().messages.orderBy('createdAt').toArray();
  },
  getAllMessagesForSession: async (sessionId: string): Promise<Message[]> => {
    return LocalDatabase.getInstance()
      .messages.where('sessionId')
      .equals(sessionId)
      .sortBy('createdAt');
  },
  getMessagesPageForSession: async (
    sessionId: string,
    page: number,
    pageSize: number,
  ): Promise<Page<Message>> => {
    const db = LocalDatabase.getInstance();
    const collection = db.messages.where({ sessionId });
    const totalItems = await collection.count();

    if (pageSize === -1) {
      const items = await collection.sortBy('createdAt');
      return createPage(items, 1, totalItems, totalItems);
    }

    const items = await collection
      .reverse()
      .offset((page - 1) * pageSize)
      .limit(pageSize)
      .sortBy('createdAt');

    return createPage(items.reverse(), page, pageSize, totalItems);
  },
  deleteAllMessagesForSession: async (sessionId: string): Promise<number> => {
    return LocalDatabase.getInstance()
      .messages.where('sessionId')
      .equals(sessionId)
      .delete();
  },
  clearAllMessages: async (): Promise<void> => {
    await LocalDatabase.getInstance().messages.clear();
  },
  bulkUpsertMessages: async (messages: Message[]): Promise<void> => {
    await dbService.messages.upsertMany(messages);
  },

  // --- File Stores ---
  getAllFileStores: async (): Promise<FileStore[]> => {
    return LocalDatabase.getInstance()
      .fileStores.orderBy('createdAt')
      .toArray();
  },
  getFileStoresBySession: async (sessionId: string): Promise<FileStore[]> => {
    return LocalDatabase.getInstance()
      .fileStores.where('sessionId')
      .equals(sessionId)
      .sortBy('createdAt');
  },
  clearAllFileStores: async (): Promise<void> => {
    const db = LocalDatabase.getInstance();
    await db.transaction(
      'rw',
      db.fileStores,
      db.fileContents,
      db.fileChunks,
      async () => {
        await db.fileChunks.clear();
        await db.fileContents.clear();
        await db.fileStores.clear();
      },
    );
  },

  // --- File Contents ---
  getFileContentsByStore: async (storeId: string): Promise<FileContent[]> => {
    return LocalDatabase.getInstance()
      .fileContents.where('storeId')
      .equals(storeId)
      .sortBy('uploadedAt');
  },
  searchFileContentsByFilename: async (
    filename: string,
  ): Promise<FileContent[]> => {
    return LocalDatabase.getInstance()
      .fileContents.where('filename')
      .startsWithIgnoreCase(filename)
      .toArray();
  },

  // --- File Chunks ---
  getFileChunksByContent: async (contentId: string): Promise<FileChunk[]> => {
    return LocalDatabase.getInstance()
      .fileChunks.where('contentId')
      .equals(contentId)
      .sortBy('chunkIndex');
  },
  getFileChunksByStore: async (storeId: string): Promise<FileChunk[]> => {
    const contents = await LocalDatabase.getInstance()
      .fileContents.where('storeId')
      .equals(storeId)
      .toArray();

    const contentIds = contents.map((content) => content.id);
    const allChunks: FileChunk[] = [];

    for (const contentId of contentIds) {
      const chunks = await LocalDatabase.getInstance()
        .fileChunks.where('contentId')
        .equals(contentId)
        .sortBy('chunkIndex');
      allChunks.push(...chunks);
    }

    return allChunks;
  },
  updateChunkEmbedding: async (
    chunkId: string,
    embedding: number[],
  ): Promise<void> => {
    await LocalDatabase.getInstance()
      .fileChunks.where('id')
      .equals(chunkId)
      .modify({ embedding });
  },
};
