import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useKnowledgeList } from './useKnowledgeList';
import { listGlobalKnowledge } from '@/lib/backend/knowledge';

vi.mock('@/lib/backend/knowledge', () => ({
  listGlobalKnowledge: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback: string) => fallback,
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
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

function createItem(id: number) {
  return {
    id,
    assistantId: 'assistant-1',
    preview: `item-${id}`,
    tags: [],
    source: null,
    createdAt: id,
  };
}

function createCursor(id: number) {
  return {
    createdAt: id,
    id,
  };
}

describe('useKnowledgeList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('ignores stale loadMore responses after the filter changes', async () => {
    let resolveStaleLoadMore: ((value: {
      items: ReturnType<typeof createItem>[];
      assistants: string[];
      nextCursor: { createdAt: number; id: number } | null;
    }) => void) | null = null;

    vi.mocked(listGlobalKnowledge).mockImplementation(async (request = {}) => {
      if (request.cursor) {
        return new Promise((resolve) => {
          resolveStaleLoadMore = resolve;
        });
      }

      if (request.assistantId === 'assistant-2') {
        return {
          items: [createItem(2)],
          assistants: ['assistant-2'],
          nextCursor: null,
        };
      }

      return {
        items: [createItem(1)],
        assistants: ['assistant-1'],
        nextCursor: createCursor(1),
      };
    });

    const { result, rerender } = renderHook(
      ({ assistantFilter }) =>
        useKnowledgeList({
          assistantFilter,
          query: '',
          refreshToken: 0,
        }),
      {
        initialProps: {
          assistantFilter: 'all',
        },
      },
    );

    await waitFor(() => {
      expect(result.current.items).toEqual([createItem(1)]);
    });

    act(() => {
      void result.current.loadMore();
    });

    rerender({ assistantFilter: 'assistant-2' });

    await waitFor(() => {
      expect(result.current.items).toEqual([createItem(2)]);
      expect(result.current.hasMoreItems).toBe(false);
    });

    act(() => {
      resolveStaleLoadMore?.({
        items: [createItem(3)],
        assistants: ['assistant-1'],
        nextCursor: createCursor(3),
      });
    });

    await waitFor(() => {
      expect(result.current.items).toEqual([createItem(2)]);
      expect(result.current.hasMoreItems).toBe(false);
      expect(result.current.isLoadingMore).toBe(false);
    });
  });
});
