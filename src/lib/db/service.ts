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
  createPage,
  fileChunksCRUD,
  fileContentsCRUD,
  fileStoresCRUD,
  groupsCRUD,
  messagesCRUD,
  objectsCRUD,
  sessionsCRUD,
} from './crud';

/**
 * A singleton class that extends Dexie to provide a local database service.
 * It defines the database schema, handles versioning, and provides access
 * to the database tables.
 */
export class LocalDatabase extends Dexie {
  private static instance: LocalDatabase;

  /**
   * Gets the singleton instance of the LocalDatabase.
   * @returns The singleton LocalDatabase instance.
   */
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

/**
 * A comprehensive database service object that exports all CRUD operations.
 * This service acts as a single point of access for all database interactions,
 * making it easy to manage data models throughout the application.
 */
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

/**
 * A collection of higher-level utility functions for interacting with the database.
 * These functions provide convenient methods for common database queries and operations
 * that are not covered by the basic CRUD interfaces.
 */
export const dbUtils = {
  // --- Assistants ---
  /**
   * Retrieves all assistants from the database, ordered by creation date.
   * @returns A promise that resolves to an array of all assistants.
   */
  getAllAssistants: async (): Promise<Assistant[]> => {
    return LocalDatabase.getInstance()
      .assistants.orderBy('createdAt')
      .toArray();
  },
  /**
   * Checks if an assistant with the given ID exists in the database.
   * @param id The ID of the assistant to check.
   * @returns A promise that resolves to true if the assistant exists, false otherwise.
   */
  assistantExists: async (id: string): Promise<boolean> => {
    return (await LocalDatabase.getInstance().assistants.get(id)) !== undefined;
  },
  /**
   * Deletes all assistants from the database.
   * @returns A promise that resolves when all assistants have been cleared.
   */
  clearAllAssistants: async (): Promise<void> => {
    await LocalDatabase.getInstance().assistants.clear();
  },
  /**
   * Inserts or updates multiple assistants in the database.
   * @param assistants An array of assistant objects to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  bulkUpsertAssistants: async (assistants: Assistant[]): Promise<void> => {
    await dbService.assistants.upsertMany(assistants);
  },

  // --- Objects ---
  /**
   * Retrieves all generic objects from the database, ordered by creation date.
   * @returns A promise that resolves to an array of all database objects.
   */
  getAllObjects: async (): Promise<DatabaseObject<unknown>[]> => {
    return LocalDatabase.getInstance().objects.orderBy('createdAt').toArray();
  },
  /**
   * Checks if an object with the given key exists in the database.
   * @param key The key of the object to check.
   * @returns A promise that resolves to true if the object exists, false otherwise.
   */
  objectExists: async (key: string): Promise<boolean> => {
    return (await LocalDatabase.getInstance().objects.get(key)) !== undefined;
  },
  /**
   * Deletes all generic objects from the database.
   * @returns A promise that resolves when all objects have been cleared.
   */
  clearAllObjects: async (): Promise<void> => {
    await LocalDatabase.getInstance().objects.clear();
  },
  /**
   * Inserts or updates multiple generic objects in the database.
   * @param objects An array of database objects to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  bulkUpsertObjects: async (objects: DatabaseObject[]): Promise<void> => {
    await dbService.objects.upsertMany(objects);
  },

  // --- Sessions ---
  /**
   * Retrieves all sessions from the database, ordered by last update time (descending).
   * @returns A promise that resolves to an array of all sessions.
   */
  getAllSessions: async (): Promise<Session[]> => {
    return LocalDatabase.getInstance()
      .sessions.orderBy('updatedAt')
      .reverse()
      .toArray();
  },
  /**
   * Deletes all sessions and their associated messages from the database.
   * @returns A promise that resolves when the operation is complete.
   */
  clearAllSessions: async (): Promise<void> => {
    await LocalDatabase.getInstance().sessions.clear();
    await LocalDatabase.getInstance().messages.clear();
  },
  /**
   * Inserts or updates multiple sessions in the database.
   * @param sessions An array of session objects to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  bulkUpsertSessions: async (sessions: Session[]): Promise<void> => {
    await dbService.sessions.upsertMany(sessions);
  },

  // --- Messages ---
  /**
   * Retrieves all messages from the database, ordered by creation date.
   * @returns A promise that resolves to an array of all messages.
   */
  getAllMessages: async (): Promise<Message[]> => {
    return LocalDatabase.getInstance().messages.orderBy('createdAt').toArray();
  },
  /**
   * Retrieves all messages for a specific session, ordered by creation date.
   * @param sessionId The ID of the session to get messages for.
   * @returns A promise that resolves to an array of messages for the session.
   */
  getAllMessagesForSession: async (sessionId: string): Promise<Message[]> => {
    return LocalDatabase.getInstance()
      .messages.where('sessionId')
      .equals(sessionId)
      .sortBy('createdAt');
  },
  /**
   * Retrieves a paginated list of messages for a specific session.
   * @param sessionId The ID of the session.
   * @param page The page number to retrieve.
   * @param pageSize The number of messages per page.
   * @returns A promise that resolves to a `Page` object containing the messages.
   */
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
  /**
   * Deletes all messages associated with a specific session.
   * @param sessionId The ID of the session whose messages should be deleted.
   * @returns A promise that resolves with the number of deleted messages.
   */
  deleteAllMessagesForSession: async (sessionId: string): Promise<number> => {
    return LocalDatabase.getInstance()
      .messages.where('sessionId')
      .equals(sessionId)
      .delete();
  },
  /**
   * Deletes all messages from the database.
   * @returns A promise that resolves when all messages have been cleared.
   */
  clearAllMessages: async (): Promise<void> => {
    await LocalDatabase.getInstance().messages.clear();
  },
  /**
   * Inserts or updates multiple messages in the database.
   * @param messages An array of message objects to upsert.
   * @returns A promise that resolves when the operation is complete.
   */
  bulkUpsertMessages: async (messages: Message[]): Promise<void> => {
    await dbService.messages.upsertMany(messages);
  },

  // --- File Stores ---
  /**
   * Retrieves all file stores from the database, ordered by creation date.
   * @returns A promise that resolves to an array of all file stores.
   */
  getAllFileStores: async (): Promise<FileStore[]> => {
    return LocalDatabase.getInstance()
      .fileStores.orderBy('createdAt')
      .toArray();
  },
  /**
   * Retrieves all file stores for a specific session, ordered by creation date.
   * @param sessionId The ID of the session.
   * @returns A promise that resolves to an array of file stores for the session.
   */
  getFileStoresBySession: async (sessionId: string): Promise<FileStore[]> => {
    return LocalDatabase.getInstance()
      .fileStores.where('sessionId')
      .equals(sessionId)
      .sortBy('createdAt');
  },
  /**
   * Deletes all file stores and their associated contents and chunks from the database.
   * @returns A promise that resolves when the operation is complete.
   */
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
  /**
   * Retrieves all file contents for a specific store, ordered by upload date.
   * @param storeId The ID of the file store.
   * @returns A promise that resolves to an array of file contents.
   */
  getFileContentsByStore: async (storeId: string): Promise<FileContent[]> => {
    return LocalDatabase.getInstance()
      .fileContents.where('storeId')
      .equals(storeId)
      .sortBy('uploadedAt');
  },
  /**
   * Searches for file contents by filename (case-insensitive prefix search).
   * @param filename The filename prefix to search for.
   * @returns A promise that resolves to an array of matching file contents.
   */
  searchFileContentsByFilename: async (
    filename: string,
  ): Promise<FileContent[]> => {
    return LocalDatabase.getInstance()
      .fileContents.where('filename')
      .startsWithIgnoreCase(filename)
      .toArray();
  },

  // --- File Chunks ---
  /**
   * Retrieves all file chunks for a specific file content, ordered by chunk index.
   * @param contentId The ID of the file content.
   * @returns A promise that resolves to an array of file chunks.
   */
  getFileChunksByContent: async (contentId: string): Promise<FileChunk[]> => {
    return LocalDatabase.getInstance()
      .fileChunks.where('contentId')
      .equals(contentId)
      .sortBy('chunkIndex');
  },
  /**
   * Retrieves all file chunks for all files within a specific store.
   * @param storeId The ID of the file store.
   * @returns A promise that resolves to a flattened array of all file chunks in the store.
   */
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
  /**
   * Updates the embedding vector for a specific file chunk.
   * @param chunkId The ID of the chunk to update.
   * @param embedding The new embedding vector.
   * @returns A promise that resolves when the chunk has been modified.
   */
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
