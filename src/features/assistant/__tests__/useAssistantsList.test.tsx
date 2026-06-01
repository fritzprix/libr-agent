import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAssistantsList } from '../hooks/useAssistantsList';
import { useAssistantContext } from '@/context/AssistantContext';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import { dbUtils } from '@/lib/db/service';
import type { BuiltinServerInfo } from '@/lib/backend/types';
import type { Assistant, MCPServerEntity } from '@/models/chat';
import { SWRConfig } from 'swr';
import React from 'react';

vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: vi.fn(),
}));

vi.mock('@/lib/backend/builtin-tools', () => ({
  listAvailableBuiltinServerDefinitions: vi.fn(),
}));

vi.mock('@/lib/db/service', () => ({
  dbUtils: {
    getMCPServersByIds: vi.fn(),
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

describe('useAssistantsList', () => {
  const mockSetPaginationMode = vi.fn();
  const mockSearchAssistants = vi.fn();
  
  const fixedDate = new Date('2026-03-13T00:00:00Z');
  
  const createMockAssistant = (id: string, name: string): Assistant => ({
    id,
    name,
    systemPrompt: 'You are an assistant',
    deletionProtected: false,
    createdAt: fixedDate,
    updatedAt: fixedDate,
    mcpServerIds: id === '1' ? ['mcp-1'] : [],
  });

  const mockAssistants = [createMockAssistant('1', 'Assistant 1')];

  const wrapper = ({ children }: { children: React.ReactNode }) => (
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      {children}
    </SWRConfig>
  );

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(useAssistantContext).mockReturnValue({
      assistants: mockAssistants,
      searchAssistants: mockSearchAssistants,
      setPaginationMode: mockSetPaginationMode,
    } as unknown as ReturnType<typeof useAssistantContext>);
    vi.mocked(listAvailableBuiltinServerDefinitions).mockResolvedValue([]);
    vi.mocked(dbUtils.getMCPServersByIds).mockResolvedValue([]);
  });

  it('sets pagination mode to paginated on mount and resets on unmount', () => {
    const { unmount } = renderHook(() => useAssistantsList(), { wrapper });
    expect(mockSetPaginationMode).toHaveBeenCalledWith('paginated');
    unmount();
    expect(mockSetPaginationMode).toHaveBeenCalledWith('full');
  });

  it('loads builtin tools map on mount', async () => {
    vi.mocked(listAvailableBuiltinServerDefinitions).mockResolvedValueOnce([
      { name: 't1', metadata: { displayName: 'Tool 1' } } as unknown as BuiltinServerInfo,
    ]);
    const { result } = renderHook(() => useAssistantsList(), { wrapper });
    await waitFor(() => {
      expect(result.current.builtinToolsMap).toEqual({ t1: 'Tool 1' });
    });
  });

  it('loads MCP servers map when assistants change', async () => {
    vi.mocked(dbUtils.getMCPServersByIds).mockResolvedValueOnce([
      { id: 'mcp-1', name: 'Server 1' } as unknown as MCPServerEntity,
    ]);
    const { result } = renderHook(() => useAssistantsList(), { wrapper });
    await waitFor(() => {
      expect(result.current.mcpServersMap).toEqual({ 'mcp-1': 'Server 1' });
    });
  });

  it('handles search and prevents stale results', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let resolveFirstSearch!: (val: Assistant[]) => void;
    mockSearchAssistants
      .mockReturnValueOnce(new Promise((res) => { resolveFirstSearch = res; }))
      .mockResolvedValueOnce([createMockAssistant('2', 'Result 2')]);

    const { result, rerender } = renderHook(() => useAssistantsList(), { wrapper });

    await act(async () => {
      result.current.handleSearch('first');
    });
    
    // Advance timers to trigger debounced query
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    // Rerender to trigger SWR re-fetch for 'first'
    rerender();
    
    await act(async () => {
      result.current.handleSearch('second');
    });
    
    // Advance timers to trigger debounced query
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    // Rerender to trigger SWR re-fetch for 'second'
    rerender();

    // Resolve first search late
    await act(async () => {
      resolveFirstSearch([createMockAssistant('1', 'Result 1')]);
    });
    
    await waitFor(() => {
      expect(result.current.isSearching).toBe(false);
    });

    // SWR handles race conditions: it only cares about the latest key ('second')
    expect(result.current.searchResults).toEqual([createMockAssistant('2', 'Result 2')]);
    vi.useRealTimers();
  });

  it('clears results when search query is emptied', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { result } = renderHook(() => useAssistantsList(), { wrapper });
    
    await act(async () => {
      result.current.handleSearch('query');
    });
    
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    await act(async () => {
      result.current.handleSearch('');
    });
    
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    expect(result.current.searchResults).toBeNull();
    expect(result.current.searchQuery).toBe('');
    vi.useRealTimers();
  });

  it('synchronizes searchQuery and searchResults to hide stale results and show searching status during debounce window', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mockSearchAssistants.mockResolvedValueOnce([createMockAssistant('2', 'Result 2')]);

    const { result } = renderHook(() => useAssistantsList(), { wrapper });

    await act(async () => {
      result.current.handleSearch('typing');
    });

    // During the debounce window:
    // searchQuery is 'typing' but debouncedSearchQuery is still ''
    expect(result.current.searchQuery).toBe('typing');
    expect(result.current.isSearching).toBe(true); // Should show loading spinner
    expect(result.current.searchResults).toBeNull(); // Should hide stale results

    // Advance fake timers by 300ms to trigger debounced query
    await act(async () => {
      vi.advanceTimersByTime(300);
    });

    // Now it should resolve
    await waitFor(() => {
      expect(result.current.isSearching).toBe(false);
    });
    expect(result.current.searchResults).toEqual([createMockAssistant('2', 'Result 2')]);
    vi.useRealTimers();
  });
});
