import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSessionHistorySearch } from '../useSessionHistorySearch';
import type { AgentSession } from '@/models/agent';

const safeInvoke = vi.fn();
const listAssistants = vi.fn();

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: (...args: unknown[]) => safeInvoke(...args),
}));

vi.mock('@/lib/backend/assistants', () => ({
  listAssistants: (...args: unknown[]) => listAssistants(...args),
}));

vi.mock('@/features/knowledge/hooks/useDebouncedValue', () => ({
  useDebouncedValue: <T,>(value: T) => value,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

function makeSession(id: string, name: string, updatedAtMs: number): AgentSession {
  return {
    id,
    name,
    status: 'idle',
    model: 'gpt',
    provider: 'openai',
    createdAt: new Date(updatedAtMs),
    updatedAt: new Date(updatedAtMs),
    isBookmarked: false,
    executionMode: 'normal',
    workspaceIsolation: 'host',
  };
}

function makeMetadata(
  id: string,
  name: string,
  updatedAt: number,
): Record<string, unknown> {
  return {
    id,
    name,
    status: 'idle',
    model: 'gpt',
    provider: 'openai',
    createdAt: updatedAt,
    updatedAt,
    isBookmarked: false,
    executionMode: 'normal',
    workspaceIsolation: 'host',
  };
}

describe('useSessionHistorySearch', () => {
  const browseSessions = [
    makeSession('browse-1', 'Browse One', 2_000),
    makeSession('browse-2', 'Browse Two', 1_000),
  ];
  const onBrowseLoadMore = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    listAssistants.mockResolvedValue([]);
  });

  it('uses browse sessions when search is empty', () => {
    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: '',
        browseSessions,
        browseHasMore: true,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    expect(result.current.isServerSearchActive).toBe(false);
    expect(result.current.displaySessions).toEqual(browseSessions);
    expect(result.current.hasMoreSessions).toBe(true);
    expect(safeInvoke).not.toHaveBeenCalled();
  });

  it('requests server search with the query', async () => {
    safeInvoke.mockResolvedValue({
      items: [makeMetadata('hit-1', 'Alpha Hit', 2_000)],
      nextCursor: { updatedAt: 2_000, id: 'hit-1' },
    });

    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: 'alpha',
        browseSessions,
        browseHasMore: true,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('agent_list_sessions', {
        request: { limit: 20, search: 'alpha' },
      });
    });

    await waitFor(() => {
      expect(result.current.displaySessions.map((session) => session.id)).toEqual([
        'hit-1',
      ]);
    });

    expect(result.current.isServerSearchActive).toBe(true);
    expect(result.current.clientSearchQuery).toBe('');
    expect(result.current.hasMoreSessions).toBe(true);
  });

  it('forwards bookmarked and status filters to the server search request', async () => {
    safeInvoke.mockResolvedValue({
      items: [makeMetadata('hit-1', 'Alpha Hit', 2_000)],
      nextCursor: undefined,
    });

    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: 'alpha',
        bookmarkedOnly: true,
        statusFilter: 'busy',
        browseSessions,
        browseHasMore: true,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('agent_list_sessions', {
        request: {
          limit: 20,
          search: 'alpha',
          bookmarkedOnly: true,
          status: 'busy',
        },
      });
    });

    await waitFor(() => {
      expect(result.current.displaySessions).toHaveLength(1);
    });
  });

  it('merges child sessions into the search result set', async () => {
    safeInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'agent_list_sessions') {
        return {
          items: [makeMetadata('parent', 'Alpha Parent', 3_000)],
          nextCursor: undefined,
        };
      }
      if (cmd === 'agent_get_child_sessions') {
        return [
          {
            ...makeMetadata('child', 'Child Session', 2_000),
            parentSessionId: 'parent',
          },
        ];
      }
      throw new Error(`unexpected command: ${cmd}`);
    });

    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: 'alpha',
        browseSessions,
        browseHasMore: false,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    await waitFor(() => {
      expect(result.current.displaySessions.map((session) => session.id)).toEqual([
        'parent',
      ]);
    });

    await act(async () => {
      await result.current.ensureSearchChildrenLoaded('parent');
    });

    await waitFor(() => {
      expect(result.current.displaySessions.map((session) => session.id)).toEqual([
        'parent',
        'child',
      ]);
    });

    expect(safeInvoke).toHaveBeenCalledWith('agent_get_child_sessions', {
      sessionId: 'parent',
    });
  });

  it('loads more search results with the same search cursor', async () => {
    safeInvoke
      .mockResolvedValueOnce({
        items: [makeMetadata('hit-1', 'Alpha One', 3_000)],
        nextCursor: { updatedAt: 3_000, id: 'hit-1' },
      })
      .mockResolvedValueOnce({
        items: [makeMetadata('hit-2', 'Alpha Two', 1_000)],
        nextCursor: undefined,
      });

    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: 'alpha',
        browseSessions,
        browseHasMore: false,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    await waitFor(() => {
      expect(result.current.displaySessions).toHaveLength(1);
    });

    await act(async () => {
      await result.current.loadMore();
    });

    await waitFor(() => {
      expect(result.current.displaySessions.map((session) => session.id)).toEqual([
        'hit-1',
        'hit-2',
      ]);
    });

    expect(safeInvoke).toHaveBeenLastCalledWith('agent_list_sessions', {
      request: {
        cursor: { updatedAt: 3_000, id: 'hit-1' },
        limit: 20,
        search: 'alpha',
      },
    });
    expect(result.current.hasMoreSessions).toBe(false);
  });

  it('removes search session trees and updates bookmarks locally', async () => {
    safeInvoke.mockResolvedValue({
      items: [
        makeMetadata('parent', 'Alpha Parent', 3_000),
        {
          ...makeMetadata('child', 'Alpha Child', 2_000),
          parentSessionId: 'parent',
        },
        makeMetadata('other', 'Alpha Other', 1_000),
      ],
      nextCursor: undefined,
    });

    const { result } = renderHook(() =>
      useSessionHistorySearch({
        searchQuery: 'alpha',
        browseSessions,
        browseHasMore: false,
        browseLoading: false,
        browseLoadingMore: false,
        onBrowseLoadMore,
      }),
    );

    await waitFor(() => {
      expect(result.current.displaySessions).toHaveLength(3);
    });

    act(() => {
      result.current.setSearchSessionBookmarked('other', true);
    });
    expect(
      result.current.displaySessions.find((session) => session.id === 'other')
        ?.isBookmarked,
    ).toBe(true);

    act(() => {
      result.current.removeSearchSessionTree('parent');
    });
    expect(result.current.displaySessions.map((session) => session.id)).toEqual([
      'other',
    ]);
  });

  it('resets isLoadingMore when a new search starts during pagination', async () => {
    safeInvoke.mockResolvedValueOnce({
      items: [makeMetadata('hit-1', 'Alpha One', 3_000)],
      nextCursor: { updatedAt: 3_000, id: 'hit-1' },
    });

    let resolveLoadMore: ((value: unknown) => void) | undefined;
    safeInvoke.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveLoadMore = resolve;
        }),
    );

    const { result, rerender } = renderHook(
      ({ query }) =>
        useSessionHistorySearch({
          searchQuery: query,
          browseSessions,
          browseHasMore: false,
          browseLoading: false,
          browseLoadingMore: false,
          onBrowseLoadMore,
        }),
      { initialProps: { query: 'alpha' } },
    );

    await waitFor(() => {
      expect(result.current.displaySessions).toHaveLength(1);
    });

    act(() => {
      void result.current.loadMore();
    });

    await waitFor(() => {
      expect(result.current.isLoadingMore).toBe(true);
    });

    safeInvoke.mockResolvedValue({
      items: [makeMetadata('hit-beta', 'Beta', 1_000)],
      nextCursor: undefined,
    });

    rerender({ query: 'beta' });

    await waitFor(() => {
      expect(result.current.isLoadingMore).toBe(false);
    });

    resolveLoadMore?.({
      items: [makeMetadata('stale', 'Stale', 1_000)],
      nextCursor: undefined,
    });

    await waitFor(() => {
      expect(result.current.displaySessions.map((session) => session.id)).toEqual([
        'hit-beta',
      ]);
    });
  });
});
