import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSkillsDirectory } from '../useSkillsDirectory';
import { safeInvoke } from '@/lib/backend/core';
import type { SkillMetadata } from '@/types/skills';

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const mockInvoke = vi.mocked(safeInvoke);

const MOCK_SKILLS: SkillMetadata[] = [
  {
    name: 'Test Skill',
    description: 'A test skill',
    path: '/skills/test/SKILL.md',
  },
];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe('useSkillsDirectory', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('requests the default directory when none is configured', async () => {
    const onSkillsDirectoryChange = vi.fn();
    mockInvoke.mockResolvedValue('/default/skills');

    renderHook(() => useSkillsDirectory(undefined, onSkillsDirectoryChange));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_default_skills_directory');
      expect(onSkillsDirectoryChange).toHaveBeenCalledWith('/default/skills');
    });
  });

  it('scans the configured directory successfully', async () => {
    const onSkillsDirectoryChange = vi.fn();
    mockInvoke.mockResolvedValue(MOCK_SKILLS);

    const { result } = renderHook(() =>
      useSkillsDirectory('/configured/skills', onSkillsDirectoryChange),
    );

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('scan_skills_directory', {
        directory: '/configured/skills',
      });
      expect(result.current.verificationStatus).toBe('success');
      expect(result.current.skills).toEqual(MOCK_SKILLS);
      expect(result.current.errorMessage).toBe('');
    });
  });

  it('clears stale errors after a successful rescan', async () => {
    const onSkillsDirectoryChange = vi.fn();
    mockInvoke.mockRejectedValueOnce(new Error('Directory not found'));
    mockInvoke.mockResolvedValueOnce(MOCK_SKILLS);

    const { result, rerender } = renderHook(
      ({ directory }: { directory?: string }) =>
        useSkillsDirectory(directory, onSkillsDirectoryChange),
      {
        initialProps: { directory: '/broken/skills' },
      },
    );

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('error');
      expect(result.current.errorMessage).toContain('Directory not found');
    });

    rerender({ directory: '/fixed/skills' });

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('success');
      expect(result.current.skills).toEqual(MOCK_SKILLS);
      expect(result.current.errorMessage).toBe('');
    });
  });

  it('ignores stale scan responses after the directory changes', async () => {
    const onSkillsDirectoryChange = vi.fn();
    const slowScan = deferred<SkillMetadata[]>();
    const fastScan = deferred<SkillMetadata[]>();

    mockInvoke.mockImplementation(
      async (command: string, args?: { directory?: string }) => {
        if (
          command === 'scan_skills_directory' &&
          args?.directory === '/slow/skills'
        ) {
          return slowScan.promise;
        }
        if (
          command === 'scan_skills_directory' &&
          args?.directory === '/fast/skills'
        ) {
          return fastScan.promise;
        }
        throw new Error(`Unexpected command: ${command}`);
      },
    );

    const { result, rerender } = renderHook(
      ({ directory }: { directory?: string }) =>
        useSkillsDirectory(directory, onSkillsDirectoryChange),
      {
        initialProps: { directory: '/slow/skills' },
      },
    );

    rerender({ directory: '/fast/skills' });

    fastScan.resolve(MOCK_SKILLS);
    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('success');
      expect(result.current.skills).toEqual(MOCK_SKILLS);
    });

    slowScan.resolve([
      {
        name: 'Stale Skill',
        description: 'Should be ignored',
        path: '/skills/stale/SKILL.md',
      },
    ]);

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(result.current.skills).toEqual(MOCK_SKILLS);
    expect(result.current.errorMessage).toBe('');
  });
});
