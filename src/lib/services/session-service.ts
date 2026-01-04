import { Session } from '@/models/chat';
import { Page } from '@/lib/db/types';
import { dbService, dbUtils } from '@/lib/db/service';
import { deleteContentStore } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';

const logger = getLogger('SessionService');

export interface ISessionService {
  getPage(page: number, pageSize: number): Promise<Page<Session>>;
  getAll(): Promise<Session[]>;
  getById(id: string): Promise<Session | undefined>;
  save(session: Session): Promise<void>;
  delete(id: string): Promise<void>;
  clearAll(): Promise<void>;
  factoryReset(): Promise<void>;
}

export class LocalSessionService implements ISessionService {
  async getPage(page: number, pageSize: number): Promise<Page<Session>> {
    return dbService.sessions.getPage(page, pageSize);
  }

  async getAll(): Promise<Session[]> {
    return dbUtils.getAllSessions();
  }

  async getById(id: string): Promise<Session | undefined> {
    return dbService.sessions.read(id);
  }

  async save(session: Session): Promise<void> {
    await dbService.sessions.upsert(session);
  }

  async delete(id: string): Promise<void> {
    // Remove backend content-store artifacts first (best-effort)
    try {
      await deleteContentStore(id);
    } catch (e) {
      logger.warn('deleteContentStore failed for session ' + id, e);
    }

    // Clear DB artifacts and native workspace (best-effort)
    try {
      await dbUtils.clearSessionAndWorkspace(id);
    } catch (e) {
      logger.warn('clearSessionAndWorkspace failed for session ' + id, e);
    }
  }

  async clearAll(): Promise<void> {
    // 1. Clear frontend DB (sessions, messages)
    await dbUtils.clearAllSessions();

    // 2. Clear backend sessions (native workspaces + sqlite data)
    try {
      // Dynamic import to avoid circular dependencies if any, though standard import is fine here
      const { clearAllSessions } = await import('@/lib/backend/sessions');
      await clearAllSessions();
    } catch (e) {
      logger.error('Failed to clear backend sessions', e);
      throw e;
    }
  }

  async factoryReset(): Promise<void> {
    // 1. Clear ALL frontend data
    try {
      await dbUtils.clearAllObjects();
      await dbUtils.clearAllSessions();
      await dbUtils.clearAllAssistants();
      await dbUtils.clearAllMCPServers();
      // Playbooks don't have a dbUtil helper explicitly shown but are in LocalDatabase
      // We can use LocalDatabase or add a helper, but referencing LocalDatabase is fine if imported
      // Actually, let's use dbService access which wraps CRUD? No, CRUD doesn't have clear.
      // Use direct dbUtils for what's available.
      // For playbooks:
      const { LocalDatabase } = await import('@/lib/db/service');
      await LocalDatabase.getInstance().playbooks.clear();
    } catch (e) {
      logger.error('Failed to clear frontend DB during factory reset', e);
    }

    // 2. Trigger backend factory reset
    try {
      const { factoryReset } = await import('@/lib/backend/sessions');
      await factoryReset();
    } catch (e) {
      logger.error('Failed to perform backend factory reset', e);
      throw e;
    }
  }
}
