import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { deleteGlobalKnowledge } from '@/lib/backend/knowledge';
import { useKnowledgeDelete } from './useKnowledgeDelete';

vi.mock('@/lib/backend/knowledge', () => ({
  deleteGlobalKnowledge: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback: string) => fallback,
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const selectedItem = {
  id: 22,
  assistantId: 'assistant-1',
  preview: 'Knowledge preview',
  tags: [],
  source: null,
  createdAt: 123,
};

describe('useKnowledgeDelete', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('confirms deletion inline and resets confirmation after success', async () => {
    const onDeleted = vi.fn();
    vi.mocked(deleteGlobalKnowledge).mockResolvedValueOnce({
      deletedChunkId: 22,
      orphanEntityCount: 1,
      orphanRelationshipCount: 2,
    });

    const { result } = renderHook(() =>
      useKnowledgeDelete({
        onDeleted,
        selectedItem,
      }),
    );

    act(() => {
      result.current.requestDelete();
    });

    expect(result.current.isDeleteConfirming).toBe(true);

    await act(async () => {
      await result.current.deleteSelectedItem();
    });

    await waitFor(() => {
      expect(deleteGlobalKnowledge).toHaveBeenCalledWith(22);
      expect(onDeleted).toHaveBeenCalledTimes(1);
      expect(result.current.isDeleteConfirming).toBe(false);
      expect(result.current.isDeleting).toBe(false);
    });
  });

  it('cancels inline confirmation without calling delete', () => {
    const { result } = renderHook(() =>
      useKnowledgeDelete({
        onDeleted: vi.fn(),
        selectedItem,
      }),
    );

    act(() => {
      result.current.requestDelete();
      result.current.cancelDelete();
    });

    expect(deleteGlobalKnowledge).not.toHaveBeenCalled();
    expect(result.current.isDeleteConfirming).toBe(false);
  });
});
