import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import { SkillsProvider, useSkills } from '../SkillsContext';

// Mock Tauri APIs
vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

// Mock useSettings
vi.mock('@/hooks/use-settings', () => ({
  useSettings: vi.fn(),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

import { safeInvoke } from '@/lib/backend/core';
import { useSettings } from '@/hooks/use-settings';
import type { SkillMetadata } from '@/types/skills';

const mockInvoke = vi.mocked(safeInvoke);
const mockUseSettings = vi.mocked(useSettings);

const MOCK_SKILLS: SkillMetadata[] = [
  {
    name: 'Test Skill',
    description: 'A test skill',
    path: '/skills/test/SKILL.md',
  },
];

function wrapper({ children }: { children: ReactNode }) {
  return <SkillsProvider>{children}</SkillsProvider>;
}

describe('SkillsContext', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((cmd: unknown) => {
      if (cmd === 'get_managed_skills_overview') {
        return Promise.resolve({
          effectiveSkills: MOCK_SKILLS,
        });
      }
      return Promise.resolve([]);
    });
  });

  describe('Initial fetch behavior', () => {
    it('calls fetchSkills after settings finish loading', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: {} },
        isLoading: false,
      } as ReturnType<typeof useSettings>);

      renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('get_managed_skills_overview');
      });
    });

    it('does NOT fetch while settings are still loading', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: {} },
        isLoading: true,
      } as ReturnType<typeof useSettings>);

      renderHook(() => useSkills(), { wrapper });

      // Give it a tick to verify nothing was called
      await new Promise((r) => setTimeout(r, 50));

      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it('uses managed skills overview regardless of skillsDirectory setting', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: { skillsDirectory: '' } },
        isLoading: false,
      } as ReturnType<typeof useSettings>);

      renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('get_managed_skills_overview');
      });
    });

    it('ignores configured skillsDirectory values and stays managed', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: { skillsDirectory: '/custom/skills/path' } },
        isLoading: false,
      } as ReturnType<typeof useSettings>);

      renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('get_managed_skills_overview');
      });
    });
  });

  describe('State management', () => {
    it('exposes scanned skills in context', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: { skillsDirectory: '/some/path' } },
        isLoading: false,
      } as ReturnType<typeof useSettings>);
      mockInvoke.mockResolvedValue({ effectiveSkills: MOCK_SKILLS });

      const { result } = renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(result.current.skills).toEqual(MOCK_SKILLS);
      });
    });

    it('sets error state on scan failure', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: { skillsDirectory: '/bad/path' } },
        isLoading: false,
      } as ReturnType<typeof useSettings>);
      mockInvoke.mockRejectedValueOnce(new Error('Directory not found'));

      const { result } = renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(result.current.error).toContain('Directory not found');
        expect(result.current.skills).toEqual([]);
      });
    });

    it('isLoading is false after fetch completes', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: {} },
        isLoading: false,
      } as ReturnType<typeof useSettings>);
      mockInvoke.mockResolvedValue({ effectiveSkills: [] });

      const { result } = renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });
    });
  });

  describe('refreshSkills', () => {
    it('re-invokes scan on refreshSkills()', async () => {
      mockUseSettings.mockReturnValue({
        value: { system: { skillsDirectory: '/my/skills' } },
        isLoading: false,
      } as ReturnType<typeof useSettings>);

      const { result } = renderHook(() => useSkills(), { wrapper });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledTimes(1);
      });

      await result.current.refreshSkills();

      expect(mockInvoke).toHaveBeenCalledTimes(2);
    });
  });

});
