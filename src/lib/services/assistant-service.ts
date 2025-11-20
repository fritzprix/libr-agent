import { Assistant } from '@/models/chat';
import { dbService, dbUtils } from '@/lib/db';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AssistantService');

export interface IAssistantService {
  getAll(): Promise<Assistant[]>;
  getById(id: string): Promise<Assistant | undefined>;
  save(assistant: Assistant): Promise<Assistant>;
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
        // Sync to local (simple overwrite for now)
        for (const assistant of remoteAssistants) {
          await this.localService.save(assistant);
        }
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

  async delete(id: string): Promise<void> {
    if (this.remoteService) {
      try {
        await this.remoteService.delete(id);
        await this.localService.delete(id);
      } catch (error) {
        logger.error('Failed to delete from remote', error);
        throw error;
      }
    } else {
      await this.localService.delete(id);
    }
  }
}
