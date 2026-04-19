import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  deleteGlobalKnowledge,
  getGlobalKnowledgeDetail,
  listGlobalKnowledge,
} from './knowledge';
import { safeInvoke } from './core';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('knowledge backend wrapper', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('listGlobalKnowledge calls safeInvoke with the request payload', async () => {
    const mockResponse = {
      items: [],
      assistants: ['assistant-1'],
      nextCursor: { createdAt: 123, id: 45 },
    };
    const request = {
      query: 'vector',
      assistantId: 'assistant-1',
      cursor: { createdAt: 120, id: 44 },
      limit: 50,
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await listGlobalKnowledge(request);

    expect(safeInvoke).toHaveBeenCalledWith('list_global_knowledge', {
      request,
    });
    expect(result).toEqual(mockResponse);
  });

  it('getGlobalKnowledgeDetail calls safeInvoke with the knowledge id', async () => {
    const mockResponse = {
      id: 42,
      assistantId: 'assistant-1',
      content: 'Stored knowledge',
      tags: ['research'],
      source: 'notes.md',
      createdAt: 123,
      primaryEntityIds: [7],
      entities: [],
      relationships: [],
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await getGlobalKnowledgeDetail(42);

    expect(safeInvoke).toHaveBeenCalledWith('get_global_knowledge_detail', {
      id: 42,
    });
    expect(result).toEqual(mockResponse);
  });

  it('deleteGlobalKnowledge calls safeInvoke with the knowledge id', async () => {
    const mockResponse = {
      deletedChunkId: 42,
      orphanEntityCount: 2,
      orphanRelationshipCount: 3,
    };
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockResponse);

    const result = await deleteGlobalKnowledge(42);

    expect(safeInvoke).toHaveBeenCalledWith('delete_global_knowledge', {
      id: 42,
    });
    expect(result).toEqual(mockResponse);
  });
});
