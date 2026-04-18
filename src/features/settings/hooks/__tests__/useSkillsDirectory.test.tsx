import { renderHook, waitFor } from '@testing-library/react';
import { SWRConfig } from 'swr';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useSkillsDirectory } from '../useSkillsDirectory';
import * as skillsBackend from '@/lib/backend/skills';

vi.mock('@/lib/backend/skills', () => ({
  getManagedSkillsOverview: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const mockOverview = {
  systemDirectory: '/system/skills',
  userDirectory: '/user/skills',
  systemSkills: [
    {
      name: 'system-skill',
      description: 'Bundled skill',
      path: '/system/skills/system-skill/SKILL.md',
      source: 'global' as const,
      origin: 'system' as const,
    },
  ],
  userSkills: [
    {
      name: 'user-skill',
      description: 'User skill',
      path: '/user/skills/user-skill/SKILL.md',
      source: 'global' as const,
      origin: 'user' as const,
    },
  ],
  effectiveSkills: [
    {
      name: 'system-skill',
      description: 'Bundled skill',
      path: '/system/skills/system-skill/SKILL.md',
      source: 'global' as const,
      origin: 'system' as const,
    },
    {
      name: 'user-skill',
      description: 'User skill',
      path: '/user/skills/user-skill/SKILL.md',
      source: 'global' as const,
      origin: 'user' as const,
    },
  ],
};

const wrapper = ({ children }: { children: ReactNode }) => (
  <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
    {children}
  </SWRConfig>
);

describe('useSkillsDirectory', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('loads the managed skills overview successfully', async () => {
    vi.mocked(skillsBackend.getManagedSkillsOverview).mockResolvedValue(
      mockOverview,
    );

    const { result } = renderHook(() => useSkillsDirectory(), { wrapper });

    await waitFor(() => {
      expect(skillsBackend.getManagedSkillsOverview).toHaveBeenCalled();
      expect(result.current.verificationStatus).toBe('success');
      expect(result.current.systemDirectory).toBe('/system/skills');
      expect(result.current.userDirectory).toBe('/user/skills');
      expect(result.current.systemSkills).toEqual(mockOverview.systemSkills);
      expect(result.current.userSkills).toEqual(mockOverview.userSkills);
      expect(result.current.skills).toEqual(mockOverview.effectiveSkills);
    });
  });

  it('surfaces backend failures as error state', async () => {
    vi.mocked(skillsBackend.getManagedSkillsOverview).mockRejectedValueOnce(
      new Error('overview failed'),
    );

    const { result } = renderHook(() => useSkillsDirectory(), { wrapper });

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('error');
      expect(result.current.errorMessage).toContain('overview failed');
      expect(result.current.skills).toEqual([]);
    });
  });

  it('refresh re-fetches the overview', async () => {
    vi.mocked(skillsBackend.getManagedSkillsOverview)
      .mockResolvedValueOnce(mockOverview)
      .mockResolvedValueOnce({
        ...mockOverview,
        userSkills: [],
        effectiveSkills: mockOverview.systemSkills,
      });

    const { result } = renderHook(() => useSkillsDirectory(), { wrapper });

    await waitFor(() => {
      expect(result.current.verificationStatus).toBe('success');
    });

    await result.current.refresh();

    await waitFor(() => {
      expect(skillsBackend.getManagedSkillsOverview).toHaveBeenCalledTimes(2);
      expect(result.current.userSkills).toEqual([]);
    });
  });
});
