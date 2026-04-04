import { renderHook, waitFor } from '@testing-library/react';
import { SWRConfig } from 'swr';
import type { ReactNode } from 'react';
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

const wrapper = ({ children }: { children: ReactNode }) => (
  <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
    {children}
  </SWRConfig>
);

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
    mockInvoke.mockResolvedValue('/default/skills');

    const { result } = renderHook(() => useSkillsDirectory(undefined), {
      wrapper,
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_default_skills_directory');
      expect(result.current.effectiveDir).toBe('/default/skills');
    });
  });

  it('scans the configured directory successfully', async () => {
    mockInvoke.mockResolvedValue(MOCK_SKILLS);

    const { result } = renderHook(
      () => useSkillsDirectory('/configured/skills'),
      { wrapper },
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
    mockInvoke.mockRejectedValueOnce(new Error('Directory not found'));
    mockInvoke.mockResolvedValueOnce(MOCK_SKILLS);

    const { result, rerender } = renderHook(
      ({ directory }: { directory?: string }) =>
        useSkillsDirectory(directory),
      {
        initialProps: { directory: '/broken/skills' },
        wrapper,
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
        useSkillsDirectory(directory),
      {
        initialProps: { directory: '/slow/skills' },
        wrapper,
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

  it('clears stale skills when a revalidation fails', async () => {
    mockInvoke.mockResolvedValueOnce(MOCK_SKILLS);
    mockInvoke.mockRejectedValueOnce(new Error('Rescan failed'));

    const { result, rerender } = renderHook(
      ({ directory }: { directory?: string }) =>
        useSkillsDirectory(directory),
      {
        initialProps: { directory: '/configured/skills' },
        wrapper,
      },
    );

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('success');
      expect(result.current.skills).toEqual(MOCK_SKILLS);
    });

    rerender({ directory: '/configured/skills-updated' });

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('error');
      expect(result.current.skills).toEqual([]);
      expect(result.current.errorMessage).toContain('Rescan failed');
    });
  });
});
