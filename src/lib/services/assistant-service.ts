import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { getLogger } from '@/lib/logger';
import { type RevalidateEvent } from './mcp-server-service';
import { RustAssistantService } from './rust-assistant-service';

// Re-export for convenience
export type { RevalidateEvent };

const logger = getLogger('AssistantService');

export interface PaginationParams {
  page: number;
  pageSize: number;
}

export interface IAssistantService {
  getAll(): Promise<Assistant[]>;
  getList(params: PaginationParams): Promise<Page<Assistant>>;
  search(query: string, limit?: number): Promise<Assistant[]>;
  getById(id: string): Promise<Assistant | undefined>;
  save(assistant: Assistant): Promise<Assistant>;
  saveAll(assistants: Assistant[]): Promise<Assistant[]>;
  delete(id: string): Promise<void>;
  onRevalidate?: (callback: (event: RevalidateEvent) => void) => () => void;
}

export class RemoteAssistantService implements IAssistantService {
  private baseUrl: string;

  constructor(baseUrl: string) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
  }

  async getAll(): Promise<Assistant[]> {
    const response = await fetch(`${this.baseUrl}/assistants`);
    if (!response.ok) throw new Error('Failed to fetch assistants');
    return response.json();
  }

  async getList(params: PaginationParams): Promise<Page<Assistant>> {
    const url = `${this.baseUrl}/assistants?page=${params.page}&pageSize=${params.pageSize}`;
    const response = await fetch(url);
    if (!response.ok) throw new Error('Failed to fetch assistants page');
    const data: unknown = await response.json();

    // If the remote API returns a Page<Assistant> shaped object, use it directly.
    // If it returns a plain array (older servers), wrap it into a Page ourselves.
    if (Array.isArray(data)) {
      const items = data as Assistant[];
      return {
        items,
        page: params.page,
        pageSize: params.pageSize,
        totalItems: items.length,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: params.page > 1,
      };
    }

    // Expect a Page<Assistant> shaped response
    const page = data as Page<Assistant>;
    return page;
  }

  async search(query: string, limit = 10): Promise<Assistant[]> {
    const response = await fetch(
      `${this.baseUrl}/assistants/search?q=${encodeURIComponent(query)}&limit=${limit}`,
    );
    if (!response.ok) throw new Error('Failed to search assistants');
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
  private localService: RustAssistantService;
  private remoteService: RemoteAssistantService | null = null;

  constructor(agentHubUrl?: string) {
    this.localService = new RustAssistantService();
    if (agentHubUrl) {
      this.remoteService = new RemoteAssistantService(agentHubUrl);
    }
  }

  /**
   * Subscribe to revalidation events from the local service
   */
  onRevalidate(callback: (event: RevalidateEvent) => void): () => void {
    return this.localService.onRevalidate(callback);
  }

  async getList(params: PaginationParams): Promise<Page<Assistant>> {
    if (this.remoteService) {
      try {
        return await this.remoteService.getList(params);
      } catch (error) {
        logger.error(
          'Failed to fetch paginated list from remote, falling back to local',
          error,
        );
        return this.localService.getList(params);
      }
    }
    return this.localService.getList(params);
  }

  async search(query: string, limit = 10): Promise<Assistant[]> {
    if (this.remoteService) {
      try {
        return await this.remoteService.search(query, limit);
      } catch (error) {
        logger.warn(
          'Failed to search from remote, falling back to local search',
          error,
        );
        return this.localService.search(query, limit);
      }
    }
    return this.localService.search(query, limit);
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
          `Failed to fetch assistant ${id} from remote, falling back to local`,
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
        // Sync to local
        await this.localService.save(saved);
        return saved;
      } catch (error) {
        logger.error('Failed to save to remote, saving to local only', error);
        return this.localService.save(assistant);
      }
    }
    return this.localService.save(assistant);
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    if (this.remoteService) {
      try {
        const saved = await this.remoteService.saveAll(assistants);
        // Sync to local
        await this.localService.saveAll(saved);
        return saved;
      } catch (error) {
        logger.error(
          'Failed to save all to remote, saving to local only',
          error,
        );
        return this.localService.saveAll(assistants);
      }
    }
    return this.localService.saveAll(assistants);
  }

  async delete(id: string): Promise<void> {
    if (this.remoteService) {
      try {
        await this.remoteService.delete(id);
        // Sync to local
        await this.localService.delete(id);
      } catch (error) {
        logger.error(
          'Failed to delete from remote, deleting from local only',
          error,
        );
        await this.localService.delete(id);
      }
    } else {
      await this.localService.delete(id);
    }
  }
}

// Export a singleton instance for backward compatibility where needed
export const assistantService = new AssistantService();
