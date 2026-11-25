import type { ReactNode } from 'react';
import { act, renderHook, waitFor } from '@testing-library/react';
import { SWRConfig } from 'swr';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useScopedSkills } from '@/features/agent/hooks/useScopedSkills';
import { useSkillsManagementPanel } from '../useSkillsManagementPanel';

const skillsBackend = vi.hoisted(() => ({
  getAggregatedSkills: vi.fn(),
  getManagedSkillsOverview: vi.fn(),
  previewGitHubSkillInstall: vi.fn(),
  installGitHubSkills: vi.fn(),
  checkPluginSourceUpdates: vi.fn(),
  deleteUserSkill: vi.fn(),
  importUserSkills: vi.fn(),
  previewUserSkillImport: vi.fn(),
  resetUserSkills: vi.fn(),
}));

vi.mock('@/lib/backend/skills', () => skillsBackend);

vi.mock('@/context/DnDContext', () => ({
  useDnDContext: () => ({ subscribe: vi.fn(() => () => {}) }),
}));

vi.mock('@/context/GlobalEventContext', () => ({
  useBackendResource: vi.fn(),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

vi.mock('sonner', () => ({
  toast: {
    loading: vi.fn(() => 'toast-id'),
    dismiss: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

function Wrapper({ children }: { children: ReactNode }) {
  return (
    <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
      {children}
    </SWRConfig>
  );
}

function usePanelWithOpenSessionStartView() {
  return {
    panel: useSkillsManagementPanel(),
    scoped: useScopedSkills('assistant-1'),
  };
}

describe('useSkillsManagementPanel scoped-skills revalidation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    skillsBackend.getManagedSkillsOverview.mockResolvedValue({
      effectiveSkills: [],
      systemSkills: [],
      userSkills: [],
      systemDirectory: '',
      userDirectory: '',
      pluginSources: [],
    });
    skillsBackend.getAggregatedSkills.mockResolvedValue([]);
    skillsBackend.previewGitHubSkillInstall.mockResolvedValue({
      kind: 'skillBundle',
      conflicts: [],
      discoveredSkills: [],
      hooks: [],
      hooksRequireApproval: false,
    });
    skillsBackend.installGitHubSkills.mockResolvedValue({
      importedNames: ['new-skill'],
      overwrittenNames: [],
      skippedNames: [],
      warnings: [],
    });
    skillsBackend.checkPluginSourceUpdates.mockResolvedValue([]);
    skillsBackend.deleteUserSkill.mockResolvedValue(undefined);
  });

  const renderOpenView = async () => {
    const view = renderHook(() => usePanelWithOpenSessionStartView(), {
      wrapper: Wrapper,
    });
    await waitFor(() =>
      expect(skillsBackend.getAggregatedSkills).toHaveBeenCalledTimes(1),
    );
    return view;
  };

  it('revalidates scoped-skills after a GitHub import', async () => {
    const { result } = await renderOpenView();
    skillsBackend.getAggregatedSkills.mockResolvedValue([
      { name: 'new-skill' },
    ]);

    act(() => {
      result.current.panel.setRepoUrl('https://github.com/example/skills');
    });
    await act(async () => {
      await result.current.panel.handleGitHubInstall();
    });

    expect(skillsBackend.installGitHubSkills).toHaveBeenCalled();
    expect(skillsBackend.getAggregatedSkills).toHaveBeenCalledTimes(2);
    await waitFor(() =>
      expect(result.current.scoped.skills).toEqual([{ name: 'new-skill' }]),
    );
  });

  it('revalidates scoped-skills after deleting a user skill', async () => {
    const { result } = await renderOpenView();

    await act(async () => {
      await result.current.panel.handleDeleteUserSkill('old-skill');
    });

    expect(skillsBackend.deleteUserSkill).toHaveBeenCalledWith('old-skill');
    expect(skillsBackend.getAggregatedSkills).toHaveBeenCalledTimes(2);
  });
});

describe('useSkillsManagementPanel plugin update indicator (#2927 D6, #2954 G1.3)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    skillsBackend.getManagedSkillsOverview.mockResolvedValue({
      effectiveSkills: [],
      systemSkills: [],
      userSkills: [],
      systemDirectory: '',
      userDirectory: '',
      pluginSources: [
        {
          id: 'src-1',
          sourceUrl: 'https://github.com/example/plugin',
          skillNames: ['plugged'],
          hookEvents: [],
          hooksApproved: true,
          enabled: true,
        },
      ],
    });
    skillsBackend.getAggregatedSkills.mockResolvedValue([]);
    skillsBackend.checkPluginSourceUpdates.mockResolvedValue([
      { sourceId: 'src-1', updateAvailable: true },
    ]);
    skillsBackend.previewGitHubSkillInstall.mockResolvedValue({
      kind: 'plugin',
      conflicts: [],
      discoveredSkills: [],
      hooks: [],
      hooksRequireApproval: false,
    });
    skillsBackend.installGitHubSkills.mockResolvedValue({
      importedNames: [],
      overwrittenNames: ['plugged'],
      skippedNames: [],
      warnings: [],
    });
  });

  it('shows the badge, then clears it after a successful update', async () => {
    const view = renderHook(() => useSkillsManagementPanel(), {
      wrapper: Wrapper,
    });
    await waitFor(() =>
      expect(skillsBackend.checkPluginSourceUpdates).toHaveBeenCalled(),
    );
    await waitFor(() =>
      expect(view.result.current.updatesAvailable).toEqual({
        'src-1': true,
      }),
    );

    await act(async () => {
      view.result.current.setPendingUpdateUrl(
        'https://github.com/example/plugin',
      );
    });
    const checksBefore = skillsBackend.checkPluginSourceUpdates.mock.calls
      .length;
    await act(async () => {
      await view.result.current.confirmPluginUpdate();
    });

    expect(skillsBackend.installGitHubSkills).toHaveBeenCalledWith(
      'https://github.com/example/plugin',
      true,
      [],
      true,
      {},
    );
    // G1.3: the update forced a second update check even though the source
    // id list did not change — without the refresh tick the effect would
    // never re-run.
    expect(skillsBackend.checkPluginSourceUpdates.mock.calls.length).toBeGreaterThan(
      checksBefore,
    );
  });

  it('clears the badge when the update check reports no update', async () => {
    const view = renderHook(() => useSkillsManagementPanel(), {
      wrapper: Wrapper,
    });
    await waitFor(() =>
      expect(skillsBackend.checkPluginSourceUpdates).toHaveBeenCalled(),
    );
    await waitFor(() =>
      expect(view.result.current.updatesAvailable).toEqual({
        'src-1': true,
      }),
    );

    skillsBackend.checkPluginSourceUpdates.mockResolvedValue([
      { sourceId: 'src-1', updateAvailable: false },
    ]);
    await act(async () => {
      view.result.current.setPendingUpdateUrl(
        'https://github.com/example/plugin',
      );
    });
    await act(async () => {
      await view.result.current.confirmPluginUpdate();
    });

    await waitFor(() =>
      expect(view.result.current.updatesAvailable).toEqual({}),
    );
  });
});
