import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAssistantsList } from '../hooks/useAssistantsList';
import { useAssistantContext } from '@/context/AssistantContext';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import { dbUtils } from '@/lib/db/service';

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
  const mockAssistants = [
    { id: '1', name: 'Assistant 1', mcpServerIds: ['mcp-1'] },
  ];

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
    const { unmount } = renderHook(() => useAssistantsList());
    expect(mockSetPaginationMode).toHaveBeenCalledWith('paginated');
    unmount();
    expect(mockSetPaginationMode).toHaveBeenCalledWith('full');
  });

  it('loads builtin tools map on mount', async () => {
    vi.mocked(listAvailableBuiltinServerDefinitions).mockResolvedValueOnce([
      { name: 't1', metadata: { displayName: 'Tool 1' } } as unknown as import('@/lib/backend/types').BuiltinServerInfo,
    ]);
    const { result } = renderHook(() => useAssistantsList());
    await waitFor(() => {
      expect(result.current.builtinToolsMap).toEqual({ t1: 'Tool 1' });
    });
  });

  it('loads MCP servers map when assistants change', async () => {
    vi.mocked(dbUtils.getMCPServersByIds).mockResolvedValueOnce([
      { id: 'mcp-1', name: 'Server 1' } as unknown as import('@/models/chat').MCPServer,
    ]);
    const { result } = renderHook(() => useAssistantsList());
    await waitFor(() => {
      expect(result.current.mcpServersMap).toEqual({ 'mcp-1': 'Server 1' });
    });
  });

  it('handles search and prevents stale results', async () => {
    let resolveFirstSearch!: (val: Assistant[]) => void;
    mockSearchAssistants
      .mockReturnValueOnce(new Promise((res) => { resolveFirstSearch = res; }))
      .mockResolvedValueOnce([{ id: '2', name: 'Result 2' }]);

    const { result } = renderHook(() => useAssistantsList());

    await act(async () => {
      result.current.handleSearch('first');
    });
    expect(result.current.isSearching).toBe(true);

    await act(async () => {
      result.current.handleSearch('second');
    });

    // Resolve first search late
    resolveFirstSearch([{ id: '1', name: 'Result 1' }]);
    
    await waitFor(() => {
      expect(result.current.isSearching).toBe(false);
    });

    // Should show results for 'second', not 'first'
    expect(result.current.searchResults).toEqual([{ id: '2', name: 'Result 2' }]);
  });

  it('clears results when search query is emptied', async () => {
    const { result } = renderHook(() => useAssistantsList());
    
    await act(async () => {
      result.current.handleSearch('query');
    });
    
    await act(async () => {
      result.current.handleSearch('');
    });
    
    expect(result.current.searchResults).toBeNull();
    expect(result.current.searchQuery).toBe('');
  });

  it('prevents state updates after unmount for MCP servers', async () => {
    let resolveMcp!: (val: import('@/models/chat').MCPServer[]) => void;
    vi.mocked(dbUtils.getMCPServersByIds).mockReturnValueOnce(
      new Promise((res) => { resolveMcp = res; })
    );

    const { unmount, result } = renderHook(() => useAssistantsList());
    
    unmount();
    resolveMcp([{ id: 'mcp-1', name: 'Server 1' }]);
    
    await new Promise(r => setTimeout(r, 10));
    expect(result.current.mcpServersMap).toEqual({});
  });
});
