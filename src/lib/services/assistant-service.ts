import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { dbService, dbUtils } from '@/lib/db';
import { createPage } from '@/lib/db/crud';
import { getLogger } from '@/lib/logger';
import { BM25Index, defaultTokenizer } from '@/lib/search/bm25';
import type { BM25Doc } from '@/lib/search/bm25';

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
}

export class LocalAssistantService implements IAssistantService {
  private searchIndex: BM25Index | null = null;
  private assistantMap: Map<string, Assistant> = new Map();
  private isIndexing = false;

  constructor() {
    // Initialize index in background (non-blocking)
    this.refreshIndex();
  }

  private async refreshIndex(): Promise<void> {
    if (this.isIndexing) return;
    this.isIndexing = true;

    try {
      const assistants = await dbUtils.getAllAssistants();

      // Build ID→Assistant Map for fast lookup during search
      this.assistantMap = new Map(assistants.map((a) => [a.id!, a]));

      // Build BM25 index with multi-field tokens
      const docs: BM25Doc[] = assistants.map((a) => ({
        id: a.id!,
        tokens: [
          ...defaultTokenizer(a.name),
          ...defaultTokenizer(a.description || ''),
          ...defaultTokenizer(a.systemPrompt),
        ],
      }));

      const newIndex = new BM25Index();
      newIndex.addDocuments(docs);
      this.searchIndex = newIndex; // Atomic replacement
    } catch (error) {
      logger.error('Failed to refresh search index', error);
    } finally {
      this.isIndexing = false;
    }
  }

  async search(query: string, limit = 10): Promise<Assistant[]> {
    // Lazy initialization: build index if missing
    if (!this.searchIndex) {
      await this.refreshIndex();
    }

    // Return empty array if index build failed
    if (!this.searchIndex) return [];

    const queryTokens = defaultTokenizer(query);
    const scores = this.searchIndex.score(queryTokens);

    // Sort by score (descending) and return top N assistants
    return Array.from(scores.entries())
      .filter(([, score]) => score > 0)
      .sort((a, b) => b[1] - a[1])
      .slice(0, limit)
      .map(([id]) => this.assistantMap.get(id)!)
      .filter(Boolean);
  }

  async getList(params: PaginationParams): Promise<Page<Assistant>> {
    const all = await dbUtils.getAllAssistants();
    const start = (params.page - 1) * params.pageSize;
    const end = start + params.pageSize;

    return createPage(
      all.slice(start, end),
      params.page,
      params.pageSize,
      all.length,
    );
  }

  async getAll(): Promise<Assistant[]> {
    return dbUtils.getAllAssistants();
  }

  async getById(id: string): Promise<Assistant | undefined> {
    return dbService.assistants.read(id);
  }

  async save(assistant: Assistant): Promise<Assistant> {
    await dbService.assistants.upsert(assistant);
    // Background index refresh (non-blocking)
    this.refreshIndex();
    return assistant;
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    if (assistants.length === 0) return [];
    await dbService.assistants.upsertMany(assistants);
    // Background index refresh (non-blocking)
    this.refreshIndex();
    return assistants;
  }

  async delete(id: string): Promise<void> {
    await dbService.assistants.delete(id);
    // Background index refresh (non-blocking)
    this.refreshIndex();
  }
}

export class RemoteAssistantService implements IAssistantService {
  constructor(private baseUrl: string) {}

  async getList(params: PaginationParams): Promise<Page<Assistant>> {
    const url = new URL(`${this.baseUrl}/assistants`);
    url.searchParams.set('page', params.page.toString());
    url.searchParams.set('pageSize', params.pageSize.toString());

    const response = await fetch(url.toString());
    if (!response.ok) throw new Error('Failed to fetch assistants');
    return response.json();
  }

  async search(query: string, limit = 10): Promise<Assistant[]> {
    const url = new URL(`${this.baseUrl}/assistants/search`);
    url.searchParams.set('q', encodeURIComponent(query));
    url.searchParams.set('limit', limit.toString());

    const response = await fetch(url.toString());
    if (response.status === 404) {
      throw new Error('Remote search endpoint not implemented');
    }
    if (!response.ok) throw new Error('Failed to search assistants');
    return response.json();
  }

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
          'Failed to search from remote, falling back to local BM25 search',
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
