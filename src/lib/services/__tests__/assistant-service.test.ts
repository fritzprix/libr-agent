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

      // Verify remote fetch was called
      expect(mockFetch).toHaveBeenCalledWith(`${agentHubUrl}/assistants`);
      // Verify local sync was NOT called (based on previous investigation of code)
      expect(mockLocalService.saveAll).not.toHaveBeenCalled();
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
