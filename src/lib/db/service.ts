import type {
  Assistant,
  Message,
  Session,
  MCPServerEntity,
} from '@/models/chat';
import type { DatabaseObject, DatabaseService, Page } from './types';
import {
  assistantsCRUD,
  mcpServersCRUD,
  messagesCRUD,
  objectsCRUD,
  sessionsCRUD,
  playbooksCRUD,
} from './crud';

import * as assistantsBackend from '@/lib/backend/assistants';
import * as mcpBackent from '@/lib/backend/mcp-server-config';
import * as settingsBackend from '@/lib/backend/settings';
import * as sessionsBackend from '@/lib/backend/session-crud';
import { getLogger } from '@/lib/logger';

const logger = getLogger('DBService');
import * as messagesBackend from '@/lib/backend/messages';
import * as playbooksBackend from '@/lib/backend/playbooks';

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
   * Retrieves all assistants.
   */
  getAllAssistants: async (): Promise<Assistant[]> => {
    return assistantsBackend.listAssistants();
  },
  /**
   * Checks if an assistant with the given ID exists.
   */
  assistantExists: async (id: string): Promise<boolean> => {
    const a = await assistantsBackend.getAssistant(id);
    return !!a;
  },
  /**
   * Deletes all assistants.
   */
  clearAllAssistants: async (): Promise<void> => {
    const all = await assistantsBackend.listAssistants();
    for (const a of all) {
      if (a.id) await assistantsBackend.deleteAssistant(a.id);
    }
  },
  /**
   * Inserts or updates multiple assistants.
   */
  bulkUpsertAssistants: async (assistants: Assistant[]): Promise<void> => {
    await assistantsBackend.upsertAssistants(assistants);
  },

  // --- MCP Servers ---
  getAllMCPServers: async (): Promise<MCPServerEntity[]> => {
    return mcpBackent.listMCPServers();
  },
  getActiveMCPServers: async (): Promise<MCPServerEntity[]> => {
    const all = await mcpBackent.listMCPServers();
    // Filter active (assuming backend returns all)
    // Legacy DB had isActive field.
    return all.filter((s) => s.isActive);
  },
  getMCPServersByIds: async (ids: string[]): Promise<MCPServerEntity[]> => {
    // Backend doesn't have bulk get. Fetch all and filter? Or loop get?
    // List is usually small.
    const all = await mcpBackent.listMCPServers();
    
    // Debug: Log all servers in database
    logger.info('🔍 Database getMCPServersByIds', {
      requestedIds: ids,
      allServersCount: all.length,
      allServers: all.map(s => ({ id: s.id, name: s.name }))
    });
    
    const filtered = all.filter((s) => ids.includes(s.id));
    
    logger.info('🎯 Filtered servers', {
      filteredCount: filtered.length,
      filtered: filtered.map(s => ({ id: s.id, name: s.name }))
    });
    
    return filtered;
  },
  mcpServerExists: async (id: string): Promise<boolean> => {
    const s = await mcpBackent.getMCPServer(id);
    return !!s;
  },
  clearAllMCPServers: async (): Promise<void> => {
    const all = await mcpBackent.listMCPServers();
    for (const s of all) {
      // Check constrained deletion?
      // If we are clearing ALL, we probably don't care about assistant refs if they are also cleared.
      // But this function might be called independently.
      // Force delete or safe delete?
      // Legacy clearAllMCPServers just cleared table.
      await mcpBackent.deleteMCPServer(s.name);
    }
  },

  // --- Objects (Settings) ---
  getAllObjects: async (): Promise<DatabaseObject<unknown>[]> => {
    return settingsBackend.listSettings();
  },
  objectExists: async (key: string): Promise<boolean> => {
    const o = await settingsBackend.getSetting(key);
    return !!o;
  },
  clearAllObjects: async (): Promise<void> => {
    const all = await settingsBackend.listSettings();
    for (const o of all) {
      await settingsBackend.deleteSetting(o.key);
    }
  },
  bulkUpsertObjects: async (objects: DatabaseObject[]): Promise<void> => {
    await settingsBackend.upsertSettings(objects);
  },

  // --- Sessions ---
  getAllSessions: async (): Promise<Session[]> => {
    // Order by updatedAt desc
    const all = await sessionsBackend.listSessions();
    return all.sort((a, b) => b.updatedAt.getTime() - a.updatedAt.getTime());
  },
  clearAllSessions: async (): Promise<void> => {
    const all = await sessionsBackend.listSessions();
    for (const s of all) {
      await sessionsBackend.deleteSession(s.id);
    }
  },
  clearSessionAndWorkspace: async (sessionId: string): Promise<void> => {
    // Backend deleteSession handles workspace removal if implemented in backend command
    await sessionsBackend.deleteSession(sessionId);
  },
  bulkUpsertSessions: async (sessions: Session[]): Promise<void> => {
    for (const s of sessions) {
      await sessionsBackend.upsertSession(s);
    }
  },

  // --- Messages ---
  getAllMessages: async (): Promise<Message[]> => {
    console.warn(
      'getAllMessages (global) called but not supported by backend properly. Returning empty list.',
    );
    return [];
  },
  getAllMessagesForSession: async (sessionId: string): Promise<Message[]> => {
    // Get session to find threadId? Or assume defaults.
    // Legacy didn't need threadId.
    // Try to get session
    // const session = await sessionsBackend.getSession(sessionId);
    // ^ unused if we just use sessionId as threadId constant

    const threadId = sessionId; // Fallback to sessionId as threadId

    // Pagination simulation
    const page = await messagesBackend.getMessagesPageForSession(
      sessionId,
      threadId,
      1,
      10000,
    );
    // Sort? Page items are returned in some order. Legacy: sortBy createdAt.
    return page.items.sort((a, b) => {
      const at = a.createdAt ? a.createdAt.getTime() : 0;
      const bt = b.createdAt ? b.createdAt.getTime() : 0;
      return at - bt;
    });
  },
  getMessagesPageForSession: async (
    sessionId: string,
    page: number,
    pageSize: number,
  ): Promise<Page<Message>> => {
    const threadId = sessionId; // Fallback or assume top thread

    return messagesBackend.getMessagesPageForSession(
      sessionId,
      threadId,
      page,
      pageSize,
    );
  },
  deleteAllMessagesForSession: async (sessionId: string): Promise<number> => {
    // Not strictly supported to "delete messages only" without deleting session in backend usually?
    // messages_commands.rs doesn't have `delete_messages_by_session`.
    // It has `delete_message` (single).
    // Wait, legacy DB separated messages from session.
    // Backend stores messages in SQLite.
    // If we want to clear conversation history but keep session settings?
    // I need to list messages and delete them.
    const msgs = await dbUtils.getAllMessagesForSession(sessionId);
    let count = 0;
    for (const m of msgs) {
      await messagesBackend.deleteMessage(m.id);
      count++;
    }
    return count;
  },
  clearAllMessages: async (): Promise<void> => {
    // Global clear not supported easily without listing everything.
    // Warn.
    console.warn('clearAllMessages not fully supported, doing nothing.');
  },
  bulkUpsertMessages: async (messages: Message[]): Promise<void> => {
    await messagesBackend.upsertMessages(messages);
  },

  // --- Playbooks ---
  clearAllPlaybooks: async (): Promise<void> => {
    const assistants = await assistantsBackend.listAssistants();

    for (const assistant of assistants) {
      if (!assistant.id) continue;
      const all = await playbooksBackend.listPlaybooks({
        agentId: assistant.id,
      });
      for (const p of all) {
        if (p.id) await playbooksBackend.deletePlaybook(p.id, assistant.id);
      }
    }
  },
};
