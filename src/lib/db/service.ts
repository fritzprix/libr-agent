import type {
  Assistant,
  Message,
  Session,
  MCPServerEntity,
} from '@/models/chat';
import Dexie, { Table } from 'dexie';
import type { DatabaseObject, DatabaseService, Page } from './types';
import type { Playbook } from '@/types/playbook';
import {
  assistantsCRUD,
  mcpServersCRUD,
  createPage,
  messagesCRUD,
  objectsCRUD,
  sessionsCRUD,
  playbooksCRUD,
} from './crud';
import { removeSession as backendRemoveSession } from '@/lib/rust-backend-client';

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
  mcpServers!: Table<MCPServerEntity, string>;
  objects!: Table<DatabaseObject<unknown>, string>;
  sessions!: Table<Session, string>;
  messages!: Table<Message, string>;
  // Groups are now handled in-memory (no persistent IndexedDB table)
  playbooks!: Table<
    Playbook & { id: string; createdAt?: Date; updatedAt?: Date },
    string
  >;
  // File content is persisted by the Rust backend; frontend no longer
  // maintains fileStores/fileContents/fileChunks in IndexedDB.

  constructor() {
    super('MCPAgentDB');

    // Consolidated schema (current version 1)
    // Historical versions (1-9) have been squashed into v1 for fresh installations
    // - Basic assistants/objects with timestamps and indices
    // - Sessions/messages support with threadId composite indices
    // - Playbooks with agentId index
    // - MCP servers table for centralized server management
    this.version(1).stores({
      assistants: '&id, createdAt, updatedAt, name',
      mcpServers: '&id, name, createdAt, updatedAt, isActive',
      objects: '&key, createdAt, updatedAt',
      sessions: '&id, createdAt, updatedAt',
      messages: '&id, sessionId, [sessionId+threadId], createdAt',
      playbooks: '&id, agentId, createdAt, updatedAt, goal',
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
  mcpServers: mcpServersCRUD,
  objects: objectsCRUD,
  sessions: sessionsCRUD,
  messages: messagesCRUD,
  playbooks: playbooksCRUD,
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

  // --- MCP Servers ---
  /**
   * Retrieves all MCP servers from the database, ordered by creation date.
   * @returns A promise that resolves to an array of all MCP servers.
   */
  getAllMCPServers: async (): Promise<MCPServerEntity[]> => {
    return LocalDatabase.getInstance()
      .mcpServers.orderBy('createdAt')
      .toArray();
  },
  /**
   * Retrieves all active MCP servers from the database, ordered by creation date.
   * @returns A promise that resolves to an array of active MCP servers.
   */
  getActiveMCPServers: async (): Promise<MCPServerEntity[]> => {
    return LocalDatabase.getInstance()
      .mcpServers.filter((s) => s.isActive)
      .toArray();
  },
  /**
   * Retrieves MCP servers by their IDs.
   * @param ids An array of MCP server IDs to retrieve.
   * @returns A promise that resolves to an array of MCP servers (may be fewer than requested if some IDs don't exist).
   */
  getMCPServersByIds: async (ids: string[]): Promise<MCPServerEntity[]> => {
    return LocalDatabase.getInstance()
      .mcpServers.where('id')
      .anyOf(ids)
      .toArray();
  },
  /**
   * Checks if an MCP server with the given ID exists in the database.
   * @param id The ID of the MCP server to check.
   * @returns A promise that resolves to true if the server exists, false otherwise.
   */
  mcpServerExists: async (id: string): Promise<boolean> => {
    return (await LocalDatabase.getInstance().mcpServers.get(id)) !== undefined;
  },
  /**
   * Deletes all MCP servers from the database.
   * @returns A promise that resolves when all servers have been cleared.
   */
  clearAllMCPServers: async (): Promise<void> => {
    await LocalDatabase.getInstance().mcpServers.clear();
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
   * Deletes all DB artifacts related to a specific session (messages, file stores,
   * file contents, chunks, and the session row), and attempts to remove the
   * native workspace directory by calling the backend `remove_session` command.
   * This function is best-effort: DB deletions are done in a transaction, and
   * workspace removal is attempted afterward; failures to remove the workspace
   * will be surfaced as errors from the backend call.
   * @param sessionId The ID of the session to delete
   */
  clearSessionAndWorkspace: async (sessionId: string): Promise<void> => {
    const db = LocalDatabase.getInstance();

    // File contents/stores are persisted by the Rust backend; frontend only
    // needs to remove message rows for the session here.

    // Delete messages and session row
    await db.transaction('rw', db.messages, db.sessions, async () => {
      await db.messages.where('sessionId').equals(sessionId).delete();
      await db.sessions.where('id').equals(sessionId).delete();
    });

    // Attempt to remove native workspace directory via backend command (best-effort)
    await backendRemoveSession(sessionId);
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

  // File content helpers removed; handled by Rust backend.
};
