import { describe, it, expect, vi, beforeEach, afterEach, type Mock } from 'vitest';
import { AssistantService } from '../assistant-service';
import { RustAssistantService } from '../rust-assistant-service';
import type { Assistant } from '@/models/chat';
import type { Page } from '@/lib/db/types';

// Mock the RustAssistantService module
vi.mock('../rust-assistant-service', () => {
  return {
    RustAssistantService: vi.fn(),
  };
});

describe('AssistantService', () => {
  let service: AssistantService;
  let mockFetch: Mock;
  let mockLocalService: {
    getAll: Mock;
    getList: Mock;
    search: Mock;
    getById: Mock;
    save: Mock;
    saveAll: Mock;
    delete: Mock;
    onRevalidate: Mock;
  };
  const agentHubUrl = 'https://hub.example.com';

  const createMockAssistant = (id: string, name: string): Assistant => ({
    id,
    name,
    systemPrompt: 'prompt',
    deletionProtected: false,
    createdAt: new Date(),
    updatedAt: new Date(),
  });

  const mockAssistants = [
    createMockAssistant('1', 'Assistant 1'),
    createMockAssistant('2', 'Assistant 2'),
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    mockFetch = vi.fn();
    vi.stubGlobal('fetch', mockFetch);

    // Define mock methods
    mockLocalService = {
      getAll: vi.fn(),
      getList: vi.fn(),
      search: vi.fn(),
      getById: vi.fn(),
      save: vi.fn(),
      saveAll: vi.fn(),
      delete: vi.fn(),
      onRevalidate: vi.fn(),
    };

    // Set implementation for the class constructor mock
    vi.mocked(RustAssistantService).mockImplementation(
      () => mockLocalService as unknown as InstanceType<typeof RustAssistantService>,
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  describe('Constructor', () => {
    it('should initialize local service when no URL is provided without triggering remote fetch', () => {
      service = new AssistantService();
      expect(RustAssistantService).toHaveBeenCalledTimes(1);
      expect(global.fetch).not.toHaveBeenCalled();
    });

    it('should initialize services when URL is provided without triggering fetch on construction', () => {
      service = new AssistantService(agentHubUrl);
      expect(RustAssistantService).toHaveBeenCalledTimes(1);
      expect(global.fetch).not.toHaveBeenCalled();
    });
  });

  describe('getAll', () => {
    it('should fetch from local when no remote service', async () => {
      service = new AssistantService();
      mockLocalService.getAll.mockResolvedValue(mockAssistants);

      const result = await service.getAll();

      expect(result).toEqual(mockAssistants);
      expect(mockLocalService.getAll).toHaveBeenCalled();
      expect(mockFetch).not.toHaveBeenCalled();
    });

    it('should fetch from remote and sync to local when remote service exists', async () => {
      service = new AssistantService(agentHubUrl);

      // Mock remote fetch success
      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants,
      });

      // Mock local saveAll success
      mockLocalService.saveAll.mockResolvedValue(mockAssistants);

      const result = await service.getAll();

      expect(result).toEqual(mockAssistants);
      expect(mockFetch).toHaveBeenCalledWith(`${agentHubUrl}/assistants`);
      expect(mockLocalService.saveAll).toHaveBeenCalledWith(mockAssistants);
    });

    it('should fallback to local when remote fetch fails', async () => {
      service = new AssistantService(agentHubUrl);

      // Mock remote fetch failure
      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
      });

      // Mock local getAll success
      mockLocalService.getAll.mockResolvedValue(mockAssistants);

      const result = await service.getAll();

      expect(result).toEqual(mockAssistants);
      expect(mockFetch).toHaveBeenCalled();
      expect(mockLocalService.getAll).toHaveBeenCalled();
    });

    it('should fallback to local when remote fetch throws', async () => {
      service = new AssistantService(agentHubUrl);

      // Mock remote fetch throw
      mockFetch.mockRejectedValue(new Error('Network error'));

      // Mock local getAll success
      mockLocalService.getAll.mockResolvedValue(mockAssistants);

      const result = await service.getAll();

      expect(result).toEqual(mockAssistants);
      expect(mockFetch).toHaveBeenCalled();
      expect(mockLocalService.getAll).toHaveBeenCalled();
    });

    it('should fallback to local getAll when remote succeeds but local saveAll fails', async () => {
      service = new AssistantService(agentHubUrl);

      // Remote fetch succeeds
      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants,
      });

      // Local saveAll throws — simulates a DB write failure
      mockLocalService.saveAll.mockRejectedValue(new Error('DB write error'));

      // Local getAll returns stale data as fallback
      const staleAssistants = [createMockAssistant('0', 'Stale Assistant')];
      mockLocalService.getAll.mockResolvedValue(staleAssistants);

      const result = await service.getAll();

      // Returns stale local data, NOT the fresh remote data
      expect(result).toEqual(staleAssistants);
      expect(mockLocalService.saveAll).toHaveBeenCalledWith(mockAssistants);
      expect(mockLocalService.getAll).toHaveBeenCalled();
    });
  });

  describe('getList', () => {
    const mockPage: Page<Assistant> = {
      items: mockAssistants,
      page: 1,
      pageSize: 10,
      totalItems: 2,
      totalPages: 1,
      hasNextPage: false,
      hasPreviousPage: false,
    };
    const params = { page: 1, pageSize: 10 };

    it('should fetch from remote when available', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants,
      });

      await service.getList(params);

      // Verify pagination params are passed as query string to remote
      expect(mockFetch).toHaveBeenCalledWith(
        `${agentHubUrl}/assistants?page=${params.page}&pageSize=${params.pageSize}`,
      );
      // No local sync for getList
      expect(mockLocalService.saveAll).not.toHaveBeenCalled();
    });

    it('should return Page-shaped response when remote returns a Page object', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockPage,
      });

      const result = await service.getList(params);

      expect(result).toEqual(mockPage);
    });

    it('should wrap plain array response into a Page when remote returns an array', async () => {
      service = new AssistantService(agentHubUrl);

      // Older remote servers may return a plain array instead of a Page object
      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants,
      });

      const result = await service.getList(params);

      expect(result.items).toEqual(mockAssistants);
      expect(result.page).toBe(params.page);
      expect(result.pageSize).toBe(params.pageSize);
    });

    it('should fallback to local when remote fails', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));
      mockLocalService.getList.mockResolvedValue(mockPage);

      const result = await service.getList(params);

      expect(result).toEqual(mockPage);
      expect(mockLocalService.getList).toHaveBeenCalledWith(params);
    });
  });

  describe('getById', () => {
    const mockAssistant = mockAssistants[0];

    it('should fetch from remote when available', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistant,
      });

      const result = await service.getById('1');

      expect(result).toEqual(mockAssistant);
      expect(mockFetch).toHaveBeenCalledWith(`${agentHubUrl}/assistants/1`);
    });

    it('should fallback to local when remote fails', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));
      mockLocalService.getById.mockResolvedValue(mockAssistant);

      const result = await service.getById('1');

      expect(result).toEqual(mockAssistant);
      expect(mockLocalService.getById).toHaveBeenCalledWith('1');
    });

    it('should return undefined if remote returns 404', async () => {
      service = new AssistantService(agentHubUrl);

      // Remote returns 404
      mockFetch.mockResolvedValue({
        ok: false,
        status: 404,
      });

      const result = await service.getById('1');

      expect(result).toBeUndefined();
      expect(mockLocalService.getById).not.toHaveBeenCalled();
    });

    it('should fallback to local when remote returns non-404 error status', async () => {
      service = new AssistantService(agentHubUrl);

      // Remote returns 500 — RemoteAssistantService throws, AssistantService catches and falls back
      mockFetch.mockResolvedValue({
        ok: false,
        status: 500,
      });

      mockLocalService.getById.mockResolvedValue(mockAssistant);

      const result = await service.getById('1');

      expect(result).toEqual(mockAssistant);
      expect(mockLocalService.getById).toHaveBeenCalledWith('1');
    });
  });

  describe('save', () => {
    const newAssistant = createMockAssistant('3', 'New Assistant');

    it('should save to remote and sync to local', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => newAssistant,
      });
      mockLocalService.save.mockResolvedValue(newAssistant);

      const result = await service.save(newAssistant);

      expect(result).toEqual(newAssistant);
      expect(mockFetch).toHaveBeenCalledWith(`${agentHubUrl}/assistants`, expect.objectContaining({
        method: 'POST',
        body: JSON.stringify(newAssistant),
      }));
      expect(mockLocalService.save).toHaveBeenCalledWith(newAssistant);
    });

    it('should save to local only if remote fails', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));
      mockLocalService.save.mockResolvedValue(newAssistant);

      const result = await service.save(newAssistant);

      expect(result).toEqual(newAssistant);
      expect(mockLocalService.save).toHaveBeenCalledWith(newAssistant);
    });
  });

  describe('saveAll', () => {
    it('should save all to local when no remote service', async () => {
      service = new AssistantService();
      mockLocalService.saveAll.mockResolvedValue(mockAssistants);

      const result = await service.saveAll(mockAssistants);

      expect(result).toEqual(mockAssistants);
      expect(mockLocalService.saveAll).toHaveBeenCalledWith(mockAssistants);
      expect(mockFetch).not.toHaveBeenCalled();
    });

    it('should save all to remote and sync to local on success', async () => {
      service = new AssistantService(agentHubUrl);

      // RemoteAssistantService.saveAll calls save() sequentially — mock each POST
      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants[0],
      });
      mockLocalService.saveAll.mockResolvedValue(mockAssistants);

      const result = await service.saveAll(mockAssistants);

      expect(mockFetch).toHaveBeenCalled();
      expect(mockLocalService.saveAll).toHaveBeenCalled();
      expect(result).toBeDefined();
    });

    it('should fallback to local saveAll when remote fails', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));
      mockLocalService.saveAll.mockResolvedValue(mockAssistants);

      const result = await service.saveAll(mockAssistants);

      expect(result).toEqual(mockAssistants);
      expect(mockLocalService.saveAll).toHaveBeenCalledWith(mockAssistants);
    });
  });

  describe('delete', () => {
    it('should delete from remote and local', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
      });

      await service.delete('1');

      expect(mockFetch).toHaveBeenCalledWith(`${agentHubUrl}/assistants/1`, expect.objectContaining({
        method: 'DELETE',
      }));
      expect(mockLocalService.delete).toHaveBeenCalledWith('1');
    });

    it('should delete from local if remote fails', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));

      await service.delete('1');

      expect(mockLocalService.delete).toHaveBeenCalledWith('1');
    });

    it('should still delete from local when remote succeeds but local delete fails', async () => {
      service = new AssistantService(agentHubUrl);

      // Remote delete succeeds
      mockFetch.mockResolvedValue({ ok: true });

      // Local delete fails on first call (after remote), succeeds on retry (in catch)
      mockLocalService.delete
        .mockRejectedValueOnce(new Error('Local DB error'))
        .mockResolvedValueOnce(undefined);

      await service.delete('1');

      // delete was called twice: once in try block, once in catch block
      expect(mockLocalService.delete).toHaveBeenCalledTimes(2);
      expect(mockLocalService.delete).toHaveBeenCalledWith('1');
    });
  });

  describe('search', () => {
    it('should search remote when available', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockResolvedValue({
        ok: true,
        json: async () => mockAssistants,
      });

      const result = await service.search('query');

      expect(result).toEqual(mockAssistants);
      expect(mockFetch).toHaveBeenCalledWith(expect.stringContaining('/assistants/search?q=query'));
    });

    it('should fallback to local search on failure', async () => {
      service = new AssistantService(agentHubUrl);

      mockFetch.mockRejectedValue(new Error('Fail'));
      mockLocalService.search.mockResolvedValue(mockAssistants);

      const result = await service.search('query');

      expect(result).toEqual(mockAssistants);
      expect(mockLocalService.search).toHaveBeenCalledWith('query', 10);
    });
  });

  describe('onRevalidate', () => {
    it('should delegate to local service', () => {
      service = new AssistantService();
      const callback = vi.fn();
      const unsubscribe = vi.fn();

      mockLocalService.onRevalidate.mockReturnValue(unsubscribe);

      const result = service.onRevalidate(callback);

      expect(mockLocalService.onRevalidate).toHaveBeenCalledWith(callback);
      expect(result).toBe(unsubscribe);
    });
  });
});
