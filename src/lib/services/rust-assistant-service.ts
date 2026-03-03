import { safeInvoke as invoke } from '@/lib/backend/core';
import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import { createPage } from '@/lib/db/crud';
import { getLogger } from '@/lib/logger';
import type {
  IAssistantService,
  PaginationParams,
  RevalidateEvent,
} from './assistant-service';

const logger = getLogger('RustAssistantService');

// Extract the config portion of Assistant (all fields except id, name, createdAt, updatedAt)
interface AssistantConfig {
  description?: string;
  avatar?: string;
  systemPrompt: string;
  mcpServerIds?: string[];
  localServices?: string[];
  allowedBuiltInServiceAliases?: string[];
  deletionProtected: boolean;
  model?: string;
  provider?: string;
  temperature?: number;
  maxTokens?: number;
}

interface AssistantDto {
  id: string;
  name: string;
  config: AssistantConfig;
  createdAt: number;
  updatedAt: number;
}

function mapDtoToAssistant(dto: AssistantDto): Assistant {
  return {
    id: dto.id,
    name: dto.name,
    ...dto.config,
    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  };
}

export class RustAssistantService implements IAssistantService {
  private revalidateCallbacks = new Set<(event: RevalidateEvent) => void>();

  async getAll(): Promise<Assistant[]> {
    try {
      const dtos = await invoke<AssistantDto[]>('list_assistants');
      return dtos.map(mapDtoToAssistant);
    } catch (error) {
      logger.error('Failed to get all assistants', error);
      throw error;
    }
  }

  async getList(params: PaginationParams): Promise<Page<Assistant>> {
    try {
      const all = await this.getAll();
      const start = (params.page - 1) * params.pageSize;
      const end = start + params.pageSize;

      return createPage(
        all.slice(start, end),
        params.page,
        params.pageSize,
        all.length,
      );
    } catch (error) {
      logger.error('Failed to get assistant list', error);
      throw error;
    }
  }

  async search(query: string, limit = 10): Promise<Assistant[]> {
    try {
      // For now, we'll do client-side filtering since we fetch all anyway.
      // In the future, we should implement server-side search.
      const all = await this.getAll();
      const lowerQuery = query.toLowerCase();

      return all
        .filter(
          (a) =>
            a.name.toLowerCase().includes(lowerQuery) ||
            a.description?.toLowerCase().includes(lowerQuery) ||
            a.systemPrompt.toLowerCase().includes(lowerQuery),
        )
        .slice(0, limit);
    } catch (error) {
      logger.error('Failed to search assistants', error);
      throw error;
    }
  }

  async getById(id: string): Promise<Assistant | undefined> {
    try {
      const dto = await invoke<AssistantDto | null>('get_assistant', { id });
      return dto ? mapDtoToAssistant(dto) : undefined;
    } catch (error) {
      logger.error(`Failed to get assistant ${id}`, error);
      throw error;
    }
  }

  async save(assistant: Assistant): Promise<Assistant> {
    try {
      // Split assistant into name and config
      const { id, name, ...config } = assistant;

      // Check if exists to decide between create and update
      // Actually, our backend commands are separate.
      // But wait, the frontend usually knows if it's new or not.
      // However, `save` implies upsert.

      // Let's try to get it first.
      const existing = await this.getById(id!);

      let resultDto: AssistantDto;

      if (existing) {
        resultDto = await invoke<AssistantDto>('update_assistant', {
          id,
          name,
          config,
        });
        this.emitRevalidate({
          entity: 'assistants',
          action: 'save',
          entityId: id!,
        });
      } else {
        resultDto = await invoke<AssistantDto>('create_assistant', {
          id: id!,
          name,
          config,
        });
        this.emitRevalidate({
          entity: 'assistants',
          action: 'save',
          entityId: id!,
        });
      }

      return mapDtoToAssistant(resultDto);
    } catch (error) {
      logger.error(`Failed to save assistant ${assistant.id}`, error);
      throw error;
    }
  }

  async saveAll(assistants: Assistant[]): Promise<Assistant[]> {
    if (assistants.length === 0) return [];

    try {
      const payloads = assistants.map(a => {
        const { id, name, ...config } = a;
        return {
          id: id!,
          name,
          config,
        };
      });

      const dtos = await safeInvoke<AssistantDto[]>('batch_upsert_assistants', { assistants: payloads });

      // Emit revalidate for each saved assistant
      for (const dto of dtos) {
        this.emitRevalidate({
          entity: 'assistants',
          action: 'save',
          entityId: dto.id,
        });
      }

      return dtos.map(mapDtoToAssistant);
    } catch (error) {
      logger.error('Failed to batch save assistants', error);
      throw error;
    }
  }

  async delete(id: string): Promise<void> {
    try {
      await invoke<void>('delete_assistant', { id });
      this.emitRevalidate({
        entity: 'assistants',
        action: 'delete',
        entityId: id,
      });
    } catch (error) {
      logger.error(`Failed to delete assistant ${id}`, error);
      throw error;
    }
  }

  onRevalidate(callback: (event: RevalidateEvent) => void): () => void {
    this.revalidateCallbacks.add(callback);
    return () => {
      this.revalidateCallbacks.delete(callback);
    };
  }

  private emitRevalidate(event: RevalidateEvent) {
    this.revalidateCallbacks.forEach((cb) => cb(event));
  }
}
