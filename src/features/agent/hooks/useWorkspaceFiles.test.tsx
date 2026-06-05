import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useWorkspaceFiles } from './useWorkspaceFiles';
import {
  listWorkspaceFilePaths,
  listWorkspaceFilePathsForPath,
} from '@/lib/backend/workspace';

vi.mock('@/lib/backend/workspace', () => ({
  listWorkspaceFilePaths: vi.fn(),
  listWorkspaceFilePathsForPath: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('useWorkspaceFiles', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('avoids redundant fetches while the query stays in the same depth bucket', async () => {
    vi.mocked(listWorkspaceFilePaths)
      .mockResolvedValueOnce(['src/App.tsx', 'src/main.tsx'])
      .mockResolvedValueOnce(['src/App.tsx', 'src/main.tsx', 'src/lib/utils.ts']);

    const { result, rerender } = renderHook(
      ({
        sessionId,
        query,
      }: {
        sessionId: string | undefined;
        query: string | null;
      }) => useWorkspaceFiles(sessionId, query),
      {
        initialProps: {
          sessionId: 'session-1',
          query: '',
        },
      },
    );

    await waitFor(() => {
      expect(result.current).toEqual(['src/App.tsx', 'src/main.tsx']);
    });

    expect(listWorkspaceFilePaths).toHaveBeenCalledTimes(1);
    expect(listWorkspaceFilePaths).toHaveBeenCalledWith('session-1', 2);

    rerender({ sessionId: 'session-1', query: 's' });
    expect(listWorkspaceFilePaths).toHaveBeenCalledTimes(1);
    expect(result.current).toEqual(['src/App.tsx', 'src/main.tsx']);

    rerender({ sessionId: 'session-1', query: 'sr' });
    expect(listWorkspaceFilePaths).toHaveBeenCalledTimes(1);
    expect(result.current).toEqual(['src/App.tsx', 'src/main.tsx']);

    rerender({ sessionId: 'session-1', query: 'src/' });

    await waitFor(() => {
      expect(listWorkspaceFilePaths).toHaveBeenCalledTimes(2);
    });
    expect(listWorkspaceFilePaths).toHaveBeenLastCalledWith('session-1', 4);
  });

  it('uses the workspace override loader when a workspace path is provided', async () => {
    vi.mocked(listWorkspaceFilePathsForPath).mockResolvedValueOnce([
      'docs/plan.md',
      'src/main.tsx',
    ]);

    const { result } = renderHook(() =>
      useWorkspaceFiles(undefined, '', '/tmp/workspace'),
    );

    await waitFor(() => {
      expect(result.current).toEqual(['docs/plan.md', 'src/main.tsx']);
    });

    expect(listWorkspaceFilePathsForPath).toHaveBeenCalledWith(
      '/tmp/workspace',
      2,
    );
    expect(listWorkspaceFilePaths).not.toHaveBeenCalled();
  });

  it('caps the results at 30 (MAX_RESULTS)', async () => {
    const manyPaths = Array.from({ length: 40 }, (_, i) => `src/file_${i}.ts`);
    vi.mocked(listWorkspaceFilePaths).mockResolvedValue(manyPaths);

    const { result } = renderHook(() =>
      useWorkspaceFiles('session-1', ''),
    );

    await waitFor(() => {
      expect(result.current.length).toBe(30);
      expect(result.current).toEqual(manyPaths.slice(0, 30));
    });
  });

  it('filters results with startsWith when query ends with a slash, otherwise uses includes', async () => {
    const paths = [
      'src/components/Button.tsx',
      'src/hooks/useWorkspaceFiles.ts',
      'components/Card.tsx',
    ];
    vi.mocked(listWorkspaceFilePaths).mockResolvedValue(paths);

    const { result, rerender } = renderHook(
      ({ query }: { query: string | null }) =>
        useWorkspaceFiles('session-1', query),
      {
        initialProps: { query: '' },
      },
    );

    await waitFor(() => {
      expect(result.current).toEqual(paths);
    });

    // Query 'components' without trailing slash -> should use includes -> matches 'src/components/...' and 'components/...'
    rerender({ query: 'components' });
    expect(result.current).toEqual([
      'src/components/Button.tsx',
      'components/Card.tsx',
    ]);

    // Query 'components/' with trailing slash -> should use startsWith -> matches 'components/Card.tsx'
    rerender({ query: 'components/' });
    expect(result.current).toEqual(['components/Card.tsx']);

    // Query 'src/' with trailing slash -> should use startsWith -> matches 'src/components/Button.tsx' and 'src/hooks/useWorkspaceFiles.ts'
    rerender({ query: 'src/' });
    expect(result.current).toEqual([
      'src/components/Button.tsx',
      'src/hooks/useWorkspaceFiles.ts',
    ]);
  });
});
