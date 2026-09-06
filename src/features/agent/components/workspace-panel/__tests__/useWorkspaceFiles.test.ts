import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  useWorkspaceFiles,
  clearWorkspaceExpandedPathsCache,
  normalizePath,
  isPathExpanded,
} from '../useWorkspaceFiles';

const mockListWorkspaceFiles = vi.fn();
const mockSession = { id: 'session-test-1' };

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    listWorkspaceFiles: mockListWorkspaceFiles,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: mockSession,
  }),
}));

vi.mock('@/hooks/use-agent-message-trigger', () => ({
  useAgentMessageTrigger: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

describe('useWorkspaceFiles path utilities', () => {
  it('normalizes paths correctly', () => {
    expect(normalizePath('./src/components/')).toBe('src/components');
    expect(normalizePath('src\\components\\Button.tsx')).toBe(
      'src/components/Button.tsx',
    );
    expect(normalizePath('./')).toBe('');
    expect(normalizePath('.')).toBe('');
    expect(normalizePath('')).toBe('');
  });

  it('checks path expansion without treating root as expanded subfolder', () => {
    const set = new Set(['src', 'src/components']);
    expect(isPathExpanded('src', set)).toBe(true);
    expect(isPathExpanded('./src', set)).toBe(true);
    expect(isPathExpanded('src/components', set)).toBe(true);
    expect(isPathExpanded('src/other', set)).toBe(false);
    expect(isPathExpanded('./', set)).toBe(false);
    expect(isPathExpanded('.', set)).toBe(false);
  });
});

describe('useWorkspaceFiles hook state and tree persistence', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearWorkspaceExpandedPathsCache();
  });

  it('loads root directory on mount', async () => {
    mockListWorkspaceFiles.mockResolvedValueOnce([
      { name: 'src', isDirectory: true },
      { name: 'package.json', isDirectory: false },
    ]);

    const { result } = renderHook(() => useWorkspaceFiles('./'));

    // Wait for initial load
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.fileTree).toHaveLength(2);
    expect(result.current.fileTree[0].name).toBe('src');
    expect(result.current.fileTree[0].isDirectory).toBe(true);
    expect(result.current.fileTree[0].isExpanded).toBe(false);
  });

  it('preserves expanded state and loads children when reloaded', async () => {
    // Root level files
    mockListWorkspaceFiles.mockImplementation(async (dirPath: string) => {
      if (dirPath === './' || dirPath === '.') {
        return [
          { name: 'src', isDirectory: true },
          { name: 'README.md', isDirectory: false },
        ];
      }
      if (dirPath === './src' || dirPath === 'src') {
        return [
          { name: 'components', isDirectory: true },
          { name: 'index.ts', isDirectory: false },
        ];
      }
      if (dirPath === './src/components' || dirPath === 'src/components') {
        return [{ name: 'Button.tsx', isDirectory: false }];
      }
      return [];
    });

    const { result } = renderHook(() => useWorkspaceFiles('./'));

    await act(async () => {
      await Promise.resolve();
    });

    // Expand 'src'
    const srcNode = result.current.fileTree.find((n) => n.name === 'src')!;
    await act(async () => {
      await result.current.toggleDirectory(srcNode);
    });

    expect(result.current.expandedPaths.has('src')).toBe(true);

    // Now reload root (simulating agent message trigger or refresh)
    await act(async () => {
      await result.current.loadDirectory('./');
    });

    // 'src' node must still be expanded and its children preserved!
    const reloadedSrc = result.current.fileTree.find((n) => n.name === 'src')!;
    expect(reloadedSrc.isExpanded).toBe(true);
    expect(reloadedSrc.children).toBeDefined();
    expect(reloadedSrc.children!.length).toBeGreaterThan(0);
    expect(reloadedSrc.children![0].name).toBe('components');
  });

  it('expandDirectory expands target directory and all ancestor paths', async () => {
    mockListWorkspaceFiles.mockImplementation(async (dirPath: string) => {
      if (dirPath === './' || dirPath === '.') {
        return [{ name: 'src', isDirectory: true }];
      }
      if (dirPath === './src' || dirPath === 'src') {
        return [{ name: 'components', isDirectory: true }];
      }
      if (dirPath === './src/components' || dirPath === 'src/components') {
        return [{ name: 'Button.tsx', isDirectory: false }];
      }
      return [];
    });

    const { result } = renderHook(() => useWorkspaceFiles('./'));

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      await result.current.expandDirectory('src/components');
    });

    expect(result.current.expandedPaths.has('src')).toBe(true);
    expect(result.current.expandedPaths.has('src/components')).toBe(true);

    const srcNode = result.current.fileTree.find((n) => n.name === 'src')!;
    expect(srcNode.isExpanded).toBe(true);
    const componentsNode = (srcNode.children ?? []).find(
      (n) => n.name === 'components',
    );
    expect(componentsNode?.isExpanded).toBe(true);
  });

  it('removes descendant paths from expandedPaths when a parent directory collapses', async () => {
    mockListWorkspaceFiles.mockImplementation(async (dirPath: string) => {
      if (dirPath === './' || dirPath === '.') {
        return [{ name: 'src', isDirectory: true }];
      }
      if (dirPath === './src' || dirPath === 'src') {
        return [{ name: 'components', isDirectory: true }];
      }
      return [];
    });

    const { result } = renderHook(() => useWorkspaceFiles('./'));

    await act(async () => {
      await Promise.resolve();
    });

    // Expand src/components directly
    await act(async () => {
      await result.current.expandDirectory('src/components');
    });

    expect(result.current.expandedPaths.has('src')).toBe(true);
    expect(result.current.expandedPaths.has('src/components')).toBe(true);

    // Now collapse 'src'
    const srcNode = result.current.fileTree.find((n) => n.name === 'src')!;
    await act(async () => {
      await result.current.toggleDirectory(srcNode);
    });

    expect(result.current.expandedPaths.has('src')).toBe(false);
    expect(result.current.expandedPaths.has('src/components')).toBe(false);
  });
});
