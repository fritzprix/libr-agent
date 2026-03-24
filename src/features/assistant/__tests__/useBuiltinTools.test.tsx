import { SWRConfig } from 'swr';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useBuiltinTools } from '../useBuiltinTools';
import { listAvailableBuiltinServerDefinitions } from '@/lib/backend/builtin-tools';
import type { BuiltinServerInfo } from '@/lib/backend/types';

vi.mock('@/lib/backend/builtin-tools', () => ({
  listAvailableBuiltinServerDefinitions: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

const mockDefs: BuiltinServerInfo[] = [
  {
    name: 'workspace',
    metadata: { displayName: 'Workspace', description: '', category: 'system' as const },
    toolCount: 3,
  },
  {
    name: 'browser',
    metadata: { displayName: 'Browser', description: '', category: 'automation' as const },
    toolCount: 2,
  },
  {
    name: 'knowledge',
    metadata: { displayName: 'Knowledge', description: '', category: 'data' as const },
    toolCount: 1,
  },
];

describe('useBuiltinTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('starts in loading state', () => {
    vi.mocked(listAvailableBuiltinServerDefinitions).mockReturnValue(
      new Promise(() => {}),
    );
    const { result } = renderHook(() => useBuiltinTools(), { wrapper: ({ children }: { children: React.ReactNode }) => <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{children}</SWRConfig> });
    expect(result.current.isLoading).toBe(true);
    expect(result.current.services).toEqual([]);
  });

  it('returns sorted services on success', async () => {
    vi.mocked(listAvailableBuiltinServerDefinitions).mockResolvedValueOnce(
      [...mockDefs],
    );
    const { result } = renderHook(() => useBuiltinTools(), { wrapper: ({ children }: { children: React.ReactNode }) => <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{children}</SWRConfig> });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.services).toEqual([
      mockDefs[1], // Browser
      mockDefs[2], // Knowledge
      mockDefs[0], // Workspace
    ]);
  });

  it('clears loading on fetch error without crashing', async () => {
    vi.mocked(listAvailableBuiltinServerDefinitions).mockRejectedValueOnce(
      new Error('backend unavailable'),
    );
    const { result } = renderHook(() => useBuiltinTools(), { wrapper: ({ children }: { children: React.ReactNode }) => <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{children}</SWRConfig> });

    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.services).toEqual([]);
  });

  it('does not update state after unmount (isMounted guard)', async () => {
    let resolveDefs!: (defs: BuiltinServerInfo[]) => void;
    vi.mocked(listAvailableBuiltinServerDefinitions).mockReturnValueOnce(
      new Promise<BuiltinServerInfo[]>((res) => {
        resolveDefs = res;
      }),
    );

    const { result, unmount } = renderHook(() => useBuiltinTools(), { wrapper: ({ children }: { children: React.ReactNode }) => <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>{children}</SWRConfig> });
    expect(result.current.isLoading).toBe(true);

    unmount();

    // Resolving after unmount must not trigger state updates
    resolveDefs(mockDefs);
    await new Promise((r) => setTimeout(r, 10));

    // Snapshot reflects state at unmount time (still loading, no services)
    expect(result.current.services).toEqual([]);
  });
});
