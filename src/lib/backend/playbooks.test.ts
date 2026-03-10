import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  createPlaybook,
  updatePlaybook,
  deletePlaybook,
  listPlaybooks,
  togglePlaybookBookmark,
  getPlaybook,
  upsertPlaybook,
  getPlaybooksPage,
} from './playbooks';
import { safeInvoke } from './core';
import type { Playbook, PlaybookStep } from '@/types/playbook';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: vi.fn(() => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  })),
}));

// Mock the schemas — mirrors real behavior: returns undefined (not null) on failure
// and only accepts strings (matching the real function signature).
vi.mock('@/lib/schemas/playbook', () => ({
  safeParsePlaybookWorkflow: vi.fn((input: string) => {
    if (input === 'invalid') return undefined;
    try {
      return JSON.parse(input) as { steps: PlaybookStep[] };
    } catch {
      return undefined;
    }
  }),
  safeParseSuccessCriteria: vi.fn((input: string) => {
    if (input === 'invalid') return undefined;
    try {
      return JSON.parse(input) as { description: string };
    } catch {
      return undefined;
    }
  }),
}));

describe('playbooks backend wrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // Fixed timestamp — avoids time-dependent test data that would make failures
  // non-reproducible across runs.
  const mockDate = 1700000000000;

  const mockStep: PlaybookStep = {
    stepId: 'step-1',
    description: 'Test step',
    action: { toolName: 'testTool', purpose: 'Verify test action' },
    requiredData: [],
    outputVariable: 'testOutput',
  };

  const mockWorkflow = { steps: [mockStep] };

  const mockDto = {
    id: 'playbook-1',
    assistantId: 'agent-1',
    goal: 'Test Goal',
    initialCommand: 'Start',
    workflow: JSON.stringify(mockWorkflow),
    successCriteria: JSON.stringify({ description: 'Done' }),
    createdAt: mockDate,
    updatedAt: mockDate,
    isBookmarked: false,
  };

  const mockPlaybook: Playbook = {
    id: 'playbook-1',
    agentId: 'agent-1',
    goal: 'Test Goal',
    initialCommand: 'Start',
    workflow: [mockStep],
    successCriteria: { description: 'Done' },
  };

  describe('createPlaybook', () => {
    it('calls safeInvoke and deserializes response', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto);

      const result = await createPlaybook(mockPlaybook);

      expect(safeInvoke).toHaveBeenCalledWith('create_playbook', {
        id: 'playbook-1',
        assistantId: 'agent-1',
        goal: 'Test Goal',
        initialCommand: 'Start',
        workflow: mockPlaybook.workflow,
        successCriteria: mockPlaybook.successCriteria,
      });

      expect(result.id).toBe('playbook-1');
      expect(result.agentId).toBe('agent-1');
      expect(result.goal).toBe('Test Goal');
      expect(result.workflow).toEqual([mockStep]);
    });

    it('handles invalid JSON strings during deserialization safely', async () => {
      const invalidDto = {
        ...mockDto,
        workflow: 'invalid',
        successCriteria: 'invalid',
      };
      vi.mocked(safeInvoke).mockResolvedValueOnce(invalidDto);

      const result = await createPlaybook(mockPlaybook);

      expect(result.workflow).toEqual([]);
      expect(result.successCriteria).toEqual({ description: '' });
    });

    it('handles object (non-string) JSON during deserialization safely', async () => {
      const objectDto = {
        ...mockDto,
        workflow: mockWorkflow,
        successCriteria: { description: 'Done' },
      };
      vi.mocked(safeInvoke).mockResolvedValueOnce(objectDto);

      const result = await createPlaybook(mockPlaybook);

      expect(result.workflow).toEqual([mockStep]);
      expect(result.successCriteria).toEqual({ description: 'Done' });
    });
  });

  describe('updatePlaybook', () => {
    it('throws error if id is missing', async () => {
      const invalidPlaybook = { ...mockPlaybook, id: undefined };
      await expect(updatePlaybook(invalidPlaybook)).rejects.toThrow('Playbook ID required for update');
    });

    it('calls safeInvoke and deserializes response', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto);

      const result = await updatePlaybook(mockPlaybook);

      expect(safeInvoke).toHaveBeenCalledWith('update_playbook', {
        id: 'playbook-1',
        assistantId: 'agent-1',
        goal: 'Test Goal',
        workflow: mockPlaybook.workflow,
        successCriteria: mockPlaybook.successCriteria,
      });

      expect(result.id).toBe('playbook-1');
    });
  });

  describe('deletePlaybook', () => {
    it('calls safeInvoke with correct arguments', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await deletePlaybook('playbook-1', 'agent-1');

      expect(safeInvoke).toHaveBeenCalledWith('delete_playbook', {
        id: 'playbook-1',
        assistantId: 'agent-1',
      });
    });
  });

  describe('listPlaybooks', () => {
    it('calls safeInvoke with correct arguments (with agentId)', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce([mockDto]);

      const result = await listPlaybooks({ agentId: 'agent-1', sortBy: 'created_at' });

      expect(safeInvoke).toHaveBeenCalledWith('list_playbooks', {
        assistantId: 'agent-1',
        sortBy: 'created_at',
      });

      expect(result).toHaveLength(1);
      expect(result[0].id).toBe('playbook-1');
    });

    it('calls safeInvoke with empty string for global listing if agentId is omitted', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce([mockDto]);

      const result = await listPlaybooks({ sortBy: 'created_at' });

      expect(safeInvoke).toHaveBeenCalledWith('list_playbooks', {
        assistantId: '',
        sortBy: 'created_at',
      });

      expect(result).toHaveLength(1);
    });
  });

  describe('togglePlaybookBookmark', () => {
    it('calls safeInvoke with correct arguments', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await togglePlaybookBookmark('playbook-1', true, 'agent-1');

      expect(safeInvoke).toHaveBeenCalledWith('toggle_playbook_bookmark', {
        id: 'playbook-1',
        assistantId: 'agent-1',
        bookmarked: true,
      });
    });
  });

  describe('getPlaybook', () => {
    it('returns deserialized playbook if found', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto);

      const result = await getPlaybook('playbook-1', 'agent-1');

      expect(safeInvoke).toHaveBeenCalledWith('get_playbook', {
        id: 'playbook-1',
        assistantId: 'agent-1',
      });
      expect(result?.id).toBe('playbook-1');
    });

    it('returns undefined if not found', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(null);

      const result = await getPlaybook('playbook-1', 'agent-1');

      expect(result).toBeUndefined();
    });
  });

  describe('upsertPlaybook', () => {
    it('throws error if id is missing', async () => {
      const invalidPlaybook = { ...mockPlaybook, id: undefined };
      await expect(upsertPlaybook(invalidPlaybook)).rejects.toThrow('Playbook ID is required for upsert');
    });

    it('calls updatePlaybook if playbook exists', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto); // getPlaybook mock
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto); // updatePlaybook mock

      await upsertPlaybook(mockPlaybook);

      expect(safeInvoke).toHaveBeenNthCalledWith(1, 'get_playbook', { id: 'playbook-1', assistantId: 'agent-1' });
      expect(safeInvoke).toHaveBeenNthCalledWith(2, 'update_playbook', expect.any(Object));
    });

    it('calls createPlaybook if playbook does not exist', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(null); // getPlaybook mock
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDto); // createPlaybook mock

      await upsertPlaybook(mockPlaybook);

      expect(safeInvoke).toHaveBeenNthCalledWith(1, 'get_playbook', { id: 'playbook-1', assistantId: 'agent-1' });
      expect(safeInvoke).toHaveBeenNthCalledWith(2, 'create_playbook', expect.any(Object));
    });
  });

  describe('getPlaybooksPage', () => {
    const mockDtos = [
      { ...mockDto, id: 'pb1' },
      { ...mockDto, id: 'pb2' },
      { ...mockDto, id: 'pb3' },
    ];

    it('returns paginated results', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDtos); // listPlaybooks mock

      const result = await getPlaybooksPage('agent-1', 1, 2);

      expect(result.items).toHaveLength(2);
      expect(result.items[0].id).toBe('pb1');
      expect(result.items[1].id).toBe('pb2');
      expect(result.page).toBe(1);
      expect(result.pageSize).toBe(2);
      expect(result.totalItems).toBe(3);
      expect(result.totalPages).toBe(2);
      expect(result.hasNextPage).toBe(true);
      expect(result.hasPreviousPage).toBe(false);
    });

    it('returns all results if pageSize is -1', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDtos); // listPlaybooks mock

      const result = await getPlaybooksPage('agent-1', 1, -1);

      expect(result.items).toHaveLength(3);
      expect(result.totalItems).toBe(3);
      expect(result.totalPages).toBe(1);
      expect(result.hasNextPage).toBe(false);
      expect(result.hasPreviousPage).toBe(false);
    });

    it('handles subsequent pages', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(mockDtos); // listPlaybooks mock

      const result = await getPlaybooksPage('agent-1', 2, 2);

      expect(result.items).toHaveLength(1);
      expect(result.items[0].id).toBe('pb3');
      expect(result.page).toBe(2);
      expect(result.hasNextPage).toBe(false);
      expect(result.hasPreviousPage).toBe(true);
    });
  });
});
