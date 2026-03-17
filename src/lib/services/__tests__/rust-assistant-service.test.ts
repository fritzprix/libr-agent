import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest';
import { RustAssistantService } from '../rust-assistant-service';
import { safeInvoke } from '@/lib/backend/core';
import { createPage } from '@/lib/db/crud';
import { getLogger } from '@/lib/logger';
import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';

// Mock dependencies
vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@/lib/db/crud', () => ({
  createPage: vi.fn(),
}));

vi.mock('@/lib/logger', () => {
  const error = vi.fn();
  return {
    getLogger: vi.fn(() => ({
      error,
      info: vi.fn(),
      warn: vi.fn(),
      debug: vi.fn(),
    })),
  };
});

describe('RustAssistantService', () => {
  let service: RustAssistantService;
  let mockLoggerError: Mock;

  const mockDateStr = '2024-01-01T00:00:00.000Z';
  const mockDateNum = new Date(mockDateStr).getTime();

  const mockAssistantDto = {
    id: '1',
    name: 'Assistant 1',
    config: {
      description: 'Desc 1',
      systemPrompt: 'Prompt 1',
      deletionProtected: false,
    },
    createdAt: mockDateNum,
    updatedAt: mockDateNum,
  };

  const mockAssistant: Assistant = {
    id: '1',
    name: 'Assistant 1',
    description: 'Desc 1',
    systemPrompt: 'Prompt 1',
    deletionProtected: false,
    createdAt: new Date(mockDateNum),
    updatedAt: new Date(mockDateNum),
  };

  const mockPage: Page<Assistant> = {
    items: [mockAssistant],
    page: 1,
    pageSize: 10,
    totalItems: 1,
    totalPages: 1,
    hasNextPage: false,
    hasPreviousPage: false,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    service = new RustAssistantService();
    mockLoggerError = vi.mocked(getLogger)('RustAssistantService').error as Mock;
    mockLoggerError.mockClear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('getAll', () => {
    it('should successfully map returned DTOs to Assistant objects', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([mockAssistantDto]);

      const result = await service.getAll();

      expect(safeInvoke).toHaveBeenCalledWith('list_assistants');
      expect(result).toEqual([mockAssistant]);
      expect(result[0].createdAt).toBeInstanceOf(Date);
      expect(result[0].updatedAt).toBeInstanceOf(Date);
    });

    it('should log and throw an error when IPC call fails', async () => {
      const error = new Error('IPC failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.getAll()).rejects.toThrow('IPC failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to get all assistants', error);
    });
  });

  describe('getList', () => {
    it('should calculate slices and pass to createPage correctly', async () => {
      const params = { page: 1, pageSize: 10 };
      vi.mocked(safeInvoke).mockResolvedValue([mockAssistantDto, mockAssistantDto]);
      vi.mocked(createPage).mockReturnValue(mockPage);

      const result = await service.getList(params);

      expect(safeInvoke).toHaveBeenCalledWith('list_assistants');
      expect(createPage).toHaveBeenCalledWith(
        [mockAssistant, mockAssistant], // start 0, end 10
        1,
        10,
        2
      );
      expect(result).toEqual(mockPage);
    });

    it('should log and throw an error when getting list fails', async () => {
      const error = new Error('Pagination failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);
      const params = { page: 1, pageSize: 10 };

      await expect(service.getList(params)).rejects.toThrow('Pagination failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to get assistant list', error);
    });
  });

  describe('search', () => {
    it('should filter correctly by name, description, and systemPrompt', async () => {
      const assistant2 = { ...mockAssistantDto, id: '2', name: 'Other Name', config: { description: 'Match Desc', systemPrompt: 'No Match', deletionProtected: false } };
      const assistant3 = { ...mockAssistantDto, id: '3', name: 'No Match', config: { description: 'No Match', systemPrompt: 'Match Prompt', deletionProtected: false } };

      vi.mocked(safeInvoke).mockResolvedValue([mockAssistantDto, assistant2, assistant3]);

      // Match name
      const nameRes = await service.search('Assistant 1');
      expect(nameRes.length).toBe(1);
      expect(nameRes[0].id).toBe('1');

      // Match desc
      const descRes = await service.search('match desc');
      expect(descRes.length).toBe(1);
      expect(descRes[0].id).toBe('2');

      // Match prompt
      const promptRes = await service.search('match prompt');
      expect(promptRes.length).toBe(1);
      expect(promptRes[0].id).toBe('3');
    });

    it('should respect the limit parameter', async () => {
      vi.mocked(safeInvoke).mockResolvedValue([mockAssistantDto, mockAssistantDto, mockAssistantDto]);

      const result = await service.search('Assistant 1', 2);
      expect(result.length).toBe(2);
    });

    it('should fall back to empty description safely', async () => {
      const noDescAssistant = { ...mockAssistantDto, config: { ...mockAssistantDto.config, description: undefined } };
      vi.mocked(safeInvoke).mockResolvedValue([noDescAssistant]);

      const result = await service.search('Assistant 1');
      expect(result.length).toBe(1);
    });

    it('should log and throw an error on failure', async () => {
      const error = new Error('Search failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.search('query')).rejects.toThrow('Search failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to search assistants', error);
    });
  });

  describe('getById', () => {
    it('should successfully retrieve and map an assistant', async () => {
      vi.mocked(safeInvoke).mockResolvedValue(mockAssistantDto);

      const result = await service.getById('1');

      expect(safeInvoke).toHaveBeenCalledWith('get_assistant', { id: '1' });
      expect(result).toEqual(mockAssistant);
    });

    it('should handle null response by returning undefined', async () => {
      vi.mocked(safeInvoke).mockResolvedValue(null);

      const result = await service.getById('999');

      expect(result).toBeUndefined();
    });

    it('should log and throw an error on failure', async () => {
      const error = new Error('Fetch failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.getById('1')).rejects.toThrow('Fetch failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to get assistant 1', error);
    });
  });

  describe('save', () => {
    it('should update existing assistant and emit revalidate', async () => {
      // Mock getById to return an existing assistant
      vi.spyOn(service, 'getById').mockResolvedValue(mockAssistant);
      vi.mocked(safeInvoke).mockResolvedValue(mockAssistantDto);

      const callback = vi.fn();
      service.onRevalidate(callback);

      const result = await service.save(mockAssistant);

      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { id, name, createdAt, updatedAt, ...config } = mockAssistant;
      expect(safeInvoke).toHaveBeenCalledWith('update_assistant', { id, name, config });
      expect(callback).toHaveBeenCalledWith({
        entity: 'assistants',
        action: 'save',
        entityId: '1',
      });
      expect(result).toEqual(mockAssistant);
    });

    it('should create new assistant and emit revalidate', async () => {
      // Mock getById to return undefined
      vi.spyOn(service, 'getById').mockResolvedValue(undefined);
      vi.mocked(safeInvoke).mockResolvedValue(mockAssistantDto);

      const callback = vi.fn();
      service.onRevalidate(callback);

      const result = await service.save(mockAssistant);

      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      const { id, name, createdAt, updatedAt, ...config } = mockAssistant;
      expect(safeInvoke).toHaveBeenCalledWith('create_assistant', { id, name, config });
      expect(callback).toHaveBeenCalledWith({
        entity: 'assistants',
        action: 'save',
        entityId: '1',
      });
      expect(result).toEqual(mockAssistant);
    });

    it('should log and throw an error on failure', async () => {
      const error = new Error('Save failed');
      vi.spyOn(service, 'getById').mockResolvedValue(undefined);
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.save(mockAssistant)).rejects.toThrow('Save failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to save assistant 1', error);
    });
  });

  describe('saveAll', () => {
    it('should return empty array immediately if input is empty', async () => {
      const result = await service.saveAll([]);
      expect(result).toEqual([]);
      expect(safeInvoke).not.toHaveBeenCalled();
    });

    it('should split assistants into payloads, call batch_upsert_assistants, and emit revalidate', async () => {
      const dtos = [mockAssistantDto, { ...mockAssistantDto, id: '2' }];
      vi.mocked(safeInvoke).mockResolvedValue(dtos);

      const callback = vi.fn();
      service.onRevalidate(callback);

      const assistantsToSave = [mockAssistant, { ...mockAssistant, id: '2' }];
      const result = await service.saveAll(assistantsToSave);

      // Verify payload shapes (strips createdAt, updatedAt)
      const expectedPayloads = assistantsToSave.map(a => {
        // eslint-disable-next-line @typescript-eslint/no-unused-vars
        const { id, name, createdAt, updatedAt, ...config } = a;
        return { id: id!, name, config };
      });

      expect(safeInvoke).toHaveBeenCalledWith('batch_upsert_assistants', { assistants: expectedPayloads });

      // Verify revalidate callback called for each dto
      expect(callback).toHaveBeenCalledTimes(2);
      expect(callback).toHaveBeenCalledWith({ entity: 'assistants', action: 'save', entityId: '1' });
      expect(callback).toHaveBeenCalledWith({ entity: 'assistants', action: 'save', entityId: '2' });

      expect(result.length).toBe(2);
      expect(result[0]).toEqual(mockAssistant);
    });

    it('should log and throw an error on failure', async () => {
      const error = new Error('Batch save failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.saveAll([mockAssistant])).rejects.toThrow('Batch save failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to batch save assistants', error);
    });
  });

  describe('delete', () => {
    it('should call delete_assistant and emit revalidate', async () => {
      vi.mocked(safeInvoke).mockResolvedValue(undefined);

      const callback = vi.fn();
      service.onRevalidate(callback);

      await service.delete('1');

      expect(safeInvoke).toHaveBeenCalledWith('delete_assistant', { id: '1' });
      expect(callback).toHaveBeenCalledWith({
        entity: 'assistants',
        action: 'delete',
        entityId: '1',
      });
    });

    it('should log and throw an error on failure', async () => {
      const error = new Error('Delete failed');
      vi.mocked(safeInvoke).mockRejectedValue(error);

      await expect(service.delete('1')).rejects.toThrow('Delete failed');
      expect(mockLoggerError).toHaveBeenCalledWith('Failed to delete assistant 1', error);
    });
  });

  describe('onRevalidate', () => {
    it('should register callback and invoke it on events', async () => {
      const callback = vi.fn();
      service.onRevalidate(callback);

      vi.mocked(safeInvoke).mockResolvedValue(undefined);
      await service.delete('1'); // triggers an event

      expect(callback).toHaveBeenCalledWith({
        entity: 'assistants',
        action: 'delete',
        entityId: '1',
      });
    });

    it('should correctly unsubscribe callback', async () => {
      const callback = vi.fn();
      const unsubscribe = service.onRevalidate(callback);

      unsubscribe();

      vi.mocked(safeInvoke).mockResolvedValue(undefined);
      await service.delete('1'); // triggers an event

      expect(callback).not.toHaveBeenCalled();
    });
  });

});
