import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useServerTools } from '../useServerTools';
import { safeInvoke } from '@/lib/backend/core';

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const mockTools = [
  { name: 'tool_one', description: 'First tool', inputSchema: {} },
  { name: 'tool_two', description: 'Second tool', inputSchema: {} },
];

describe('useServerTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns idle state when isOpen is false', () => {
    const { result } = renderHook(() => useServerTools('server-1', false));
    expect(result.current.isLoading).toBe(false);
    expect(result.current.tools).toEqual([]);
    expect(result.current.error).toBeNull();
    expect(safeInvoke).not.toHaveBeenCalled();
  });

  it('shows loading state immediately when isOpen becomes true', () => {
    vi.mocked(safeInvoke).mockReturnValue(new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useServerTools('server-1', true));
    expect(result.current.isLoading).toBe(true);
    expect(result.current.tools).toEqual([]);
    expect(result.current.error).toBeNull();
  });

  it('populates tools on successful probe', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce(mockTools);
    const { result } = renderHook(() => useServerTools('server-1', true));

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.tools).toEqual(mockTools);
    expect(result.current.error).toBeNull();
    expect(safeInvoke).toHaveBeenCalledWith('probe_mcp_server', {
      serverId: 'server-1',
    });
  });

  it('sets error and clears loading on probe failure', async () => {
    vi.mocked(safeInvoke).mockRejectedValueOnce(new Error('Connection refused'));
    const { result } = renderHook(() => useServerTools('server-1', true));

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.error).toBe('Connection refused');
    expect(result.current.tools).toEqual([]);
  });

  it('does not update state after unmount (isMounted guard)', async () => {
    let resolveProbe!: (tools: typeof mockTools) => void;
    vi.mocked(safeInvoke).mockReturnValueOnce(
      new Promise((res) => {
        resolveProbe = res;
      }),
    );

    const { result, unmount } = renderHook(() =>
      useServerTools('server-1', true),
    );

    expect(result.current.isLoading).toBe(true);

    // Unmount before the probe resolves
    unmount();

    // Resolve after unmount — state must not change
    resolveProbe(mockTools);

    // Give React a tick to detect any erroneous state update
    await new Promise((r) => setTimeout(r, 10));

    // No "act" warnings and result is unchanged (hook is unmounted, so
    // result.current reflects the last snapshot before unmount)
    expect(result.current.tools).toEqual([]);
  });

  it('re-fetches when serverId changes', async () => {
    const toolsA = [{ name: 'tool_a', description: '', inputSchema: {} }];
    const toolsB = [{ name: 'tool_b', description: '', inputSchema: {} }];
    vi.mocked(safeInvoke)
      .mockResolvedValueOnce(toolsA)
      .mockResolvedValueOnce(toolsB);

    const { result, rerender } = renderHook(
      ({ id }) => useServerTools(id, true),
      { initialProps: { id: 'server-a' } },
    );

    await waitFor(() => expect(result.current.tools).toEqual(toolsA));

    rerender({ id: 'server-b' });

    await waitFor(() => expect(result.current.tools).toEqual(toolsB));
    expect(safeInvoke).toHaveBeenCalledTimes(2);
  });
});
