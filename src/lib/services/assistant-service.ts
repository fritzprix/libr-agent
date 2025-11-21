import { Assistant } from '@/models/chat';
import { dbService, dbUtils } from '@/lib/db';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AssistantService');

export interface IAssistantService {
  getAll(): Promise<Assistant[]>;
  getById(id: string): Promise<Assistant | undefined>;
  save(assistant: Assistant): Promise<Assistant>;
  saveAll(assistants: Assistant[]): Promise<Assistant[]>;
  delete(id: string): Promise<void>;
}

export class LocalAssistantService implements IAssistantService {
  async getAll(): Promise<Assistant[]> {
    return dbUtils.getAllAssistants();
  }

  async getById(id: string): Promise<Assistant | undefined> {
    return dbService.assistants.read(id);
  }

  async save(assistant: Assistant): Promise<Assistant> {
    await dbService.assistants.upsert(assistant);
    return assistant;
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    if (assistants.length === 0) return [];
    await dbService.assistants.upsertMany(assistants);
    return assistants;
  }

  async delete(id: string): Promise<void> {
    await dbService.assistants.delete(id);
  }
}

export class RemoteAssistantService implements IAssistantService {
  constructor(private baseUrl: string) {}

  async getAll(): Promise<Assistant[]> {
    const response = await fetch(`${this.baseUrl}/assistants`);
    if (!response.ok) throw new Error('Failed to fetch assistants');
    return response.json();
  }

  async getById(id: string): Promise<Assistant | undefined> {
    const response = await fetch(`${this.baseUrl}/assistants/${id}`);
    if (response.status === 404) return undefined;
    if (!response.ok) throw new Error('Failed to fetch assistant');
    return response.json();
  }

  async save(assistant: Assistant): Promise<Assistant> {
    const response = await fetch(`${this.baseUrl}/assistants`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(assistant),
    });
    if (!response.ok) throw new Error('Failed to save assistant');
    return response.json();
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    // Remote API doesn't support bulk save yet, so we do sequential
    // This is mainly used for sync to local, so this path might not be used often
    const saved: Assistant[] = [];
    for (const assistant of assistants) {
      saved.push(await this.save(assistant));
    }
    return saved;
  }

  async delete(id: string): Promise<void> {
    const response = await fetch(`${this.baseUrl}/assistants/${id}`, {
      method: 'DELETE',
    });
    if (!response.ok) throw new Error('Failed to delete assistant');
  }
}

export class AssistantService implements IAssistantService {
  private localService: LocalAssistantService;
  private remoteService: RemoteAssistantService | null = null;

  constructor(agentHubUrl?: string) {
    this.localService = new LocalAssistantService();
    if (agentHubUrl) {
      this.remoteService = new RemoteAssistantService(agentHubUrl);
    }
  }

  async getAll(): Promise<Assistant[]> {
    if (this.remoteService) {
      try {
        const remoteAssistants = await this.remoteService.getAll();
        // Sync to local (batch operation)
        await this.localService.saveAll(remoteAssistants);
        return remoteAssistants;
      } catch (error) {
        logger.error(
          'Failed to fetch from remote, falling back to local',
          error,
        );
        return this.localService.getAll();
      }
    }
    return this.localService.getAll();
  }

  async getById(id: string): Promise<Assistant | undefined> {
    if (this.remoteService) {
      try {
        return await this.remoteService.getById(id);
      } catch (error) {
        logger.error(
          'Failed to fetch from remote, falling back to local',
          error,
        );
        return this.localService.getById(id);
      }
    }
    return this.localService.getById(id);
  }

  async save(assistant: Assistant): Promise<Assistant> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.save(assistant);
        await this.localService.save(saved);
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    return this.localService.save(assistant);
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.saveAll(assistants);
        await this.localService.saveAll(saved);
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote', error);
        throw error;
      }
    }
    return this.localService.saveAll(assistants);
  }

  async delete(id: string): Promise<void> {
    if (this.remoteService) {
      try {
        await this.remoteService.delete(id);
      } catch (error) {
        logger.error('Failed to delete from remote', error);
        throw error;
      }

      try {
        await this.localService.delete(id);
      } catch (error) {
        logger.error(
          'Remote deletion succeeded, but failed to delete from local',
          error,
        );
        // Do not throw; treat as success, but log for reconciliation
      }
    } else {
      await this.localService.delete(id);
    }
  }
}
