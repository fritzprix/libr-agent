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
        try {
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
        } catch (error) {
            logger.error(`Unexpected error while deleting session ${id}`, error);
            throw error;
        }
    }

    async clearAll(): Promise<void> {
        // Collect existing session ids so we can attempt to remove native workspaces
        const sessions = await dbUtils.getAllSessions();

        // Clear sessions/messages in DB in one operation first to ensure any
        // concurrent SWR revalidation will see an empty DB.
        await dbUtils.clearAllSessions();

        // Attempt native workspace removal for each previously-known session id
        for (const s of sessions) {
            try {
                await deleteContentStore(s.id);
            } catch (e) {
                logger.warn('deleteContentStore failed for session ' + s.id, e);
            }
        }
    }
}
