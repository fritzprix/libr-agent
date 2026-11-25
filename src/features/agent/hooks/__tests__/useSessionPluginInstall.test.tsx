import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SkillImportPreview } from '@/types/skills';
import { useSessionPluginInstall } from '../useSessionPluginInstall';

const mockState = vi.hoisted(() => ({
  preview: vi.fn(),
  install: vi.fn(),
  previewGitHub: vi.fn(),
  installGitHub: vi.fn(),
  resolveTarget: vi.fn(),
  mutate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: mockState.toastSuccess,
    error: mockState.toastError,
    loading: () => 'toast-id',
    dismiss: vi.fn(),
  },
}));

vi.mock('swr', () => ({
  useSWRConfig: () => ({ mutate: mockState.mutate }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/lib/backend/skills', () => ({
  previewSessionPluginInstall: (...args: unknown[]) =>
    mockState.preview(...args),
  installSessionPlugin: (...args: unknown[]) => mockState.install(...args),
  previewGitHubSkillInstall: (...args: unknown[]) =>
    mockState.previewGitHub(...args),
  installGitHubSkills: (...args: unknown[]) =>
    mockState.installGitHub(...args),
  resolvePluginTarget: (...args: unknown[]) =>
    mockState.resolveTarget(...args),
}));

const URL = 'https://github.com/o/r';

function preview(overrides: Partial<SkillImportPreview>): SkillImportPreview {
  return {
    kind: 'skillBundle',
    discoveredSkills: [{ name: 'alpha', description: 'a' }],
    conflicts: [],
    hooks: [],
    hooksRequireApproval: false,
    ...overrides,
  };
}

describe('useSessionPluginInstall', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState.mutate.mockResolvedValue(undefined);
    mockState.install.mockResolvedValue({
      importedNames: ['alpha'],
      overwrittenNames: [],
      skippedNames: [],
    });
    mockState.installGitHub.mockResolvedValue({
      importedNames: ['alpha'],
      overwrittenNames: [],
      skippedNames: [],
      warnings: [],
    });
    mockState.resolveTarget.mockResolvedValue({
      sourceUrl: URL,
      scope: 'session',
    });
  });

  it('installs immediately when there is nothing to approve', async () => {
    mockState.preview.mockResolvedValue(preview({}));
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    let consumed = false;
    await act(async () => {
      consumed = await result.current.startInstall(URL);
    });

    expect(consumed).toBe(true);
    expect(mockState.install).toHaveBeenCalledWith(
      's1',
      URL,
      false,
      [],
      false,
      {},
    );
    expect(result.current.dialogProps.pendingInstall).toBeNull();
  });

  it('opens the shared approval dialog for hook-bearing plugins and installs on confirm', async () => {
    mockState.preview.mockResolvedValue(
      preview({
        kind: 'plugin',
        hooksRequireApproval: true,
        hooks: [
          {
            event: 'SessionStart',
            command: 'echo hi',
            executionStatus: 'executed',
          },
        ],
      }),
    );
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    await act(async () => {
      await result.current.startInstall(URL);
    });
    expect(result.current.dialogProps.pendingInstall?.sourceValue).toBe(URL);
    expect(mockState.install).not.toHaveBeenCalled();

    act(() => {
      result.current.dialogProps.onApproveHooksChange(true);
    });
    await act(async () => {
      await result.current.dialogProps.onConfirm();
    });

    expect(mockState.install).toHaveBeenCalledWith(
      's1',
      URL,
      false,
      [],
      true,
      {},
    );
    expect(result.current.dialogProps.pendingInstall).toBeNull();
  });

  it('excludes system conflicts and unchecked overwrites', async () => {
    mockState.preview.mockResolvedValue(
      preview({
        conflicts: [
          { name: 'sys', existingOrigin: 'system', existingPath: '/sys' },
          { name: 'mine', existingOrigin: 'workspace', existingPath: '/ws' },
        ],
      }),
    );
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    await act(async () => {
      await result.current.startInstall(URL);
    });
    act(() => {
      result.current.dialogProps.onToggleOverwrite('mine');
    });
    await act(async () => {
      await result.current.dialogProps.onConfirm();
    });

    expect(mockState.install).toHaveBeenCalledWith(
      's1',
      URL,
      false,
      ['sys', 'mine'],
      false,
      {},
    );
  });

  it('does nothing without a session', async () => {
    const { result } = renderHook(() => useSessionPluginInstall(undefined));
    let consumed = true;
    await act(async () => {
      consumed = await result.current.startInstall(URL);
    });
    expect(consumed).toBe(false);
    expect(mockState.preview).not.toHaveBeenCalled();
  });

  // --- `/plugin update` (#2927 D6, #2954 G1.5-FE) -------------------------

  it('startUpdate resolves the target and opens the update notice', async () => {
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    let consumed = false;
    await act(async () => {
      consumed = await result.current.startUpdate('my-plugin');
    });

    expect(consumed).toBe(true);
    expect(mockState.resolveTarget).toHaveBeenCalledWith('s1', 'my-plugin');
    expect(result.current.updateNoticeProps.pendingUpdateUrl).toBe(URL);
    expect(mockState.install).not.toHaveBeenCalled();
  });

  it('runUpdateAfterAgree installs with approveHooks=true and excludes system conflicts', async () => {
    mockState.preview.mockResolvedValue(
      preview({
        conflicts: [
          { name: 'sys', existingOrigin: 'system', existingPath: '/sys' },
        ],
      }),
    );
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    await act(async () => {
      await result.current.startUpdate('my-plugin');
    });
    await act(async () => {
      await result.current.updateNoticeProps.onAgree();
    });

    expect(mockState.install).toHaveBeenCalledWith(
      's1',
      URL,
      false,
      ['sys'],
      true,
      {},
    );
    expect(mockState.installGitHub).not.toHaveBeenCalled();
    expect(result.current.updateNoticeProps.pendingUpdateUrl).toBeNull();
    expect(result.current.dialogProps.pendingInstall).toBeNull();
  });

  it('global-scope update uses the shared GitHub path and signals the D6 badge', async () => {
    mockState.resolveTarget.mockResolvedValue({
      sourceUrl: URL,
      scope: 'global',
    });
    mockState.previewGitHub.mockResolvedValue(preview({}));
    const dispatchSpy = vi.spyOn(window, 'dispatchEvent');
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    await act(async () => {
      await result.current.startUpdate('my-plugin');
    });
    await act(async () => {
      await result.current.updateNoticeProps.onAgree();
    });

    expect(mockState.install).not.toHaveBeenCalled();
    expect(mockState.installGitHub).toHaveBeenCalledWith(URL, true, [], true, {});
    const events = dispatchSpy.mock.calls.map((call) => call[0].type);
    expect(events).toContain('libragent:plugin-updated');
    dispatchSpy.mockRestore();
  });

  it('update with reviewable conflicts opens the dialog with approveHooks preset', async () => {
    mockState.preview.mockResolvedValue(
      preview({
        conflicts: [
          { name: 'mine', existingOrigin: 'workspace', existingPath: '/ws' },
        ],
      }),
    );
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    await act(async () => {
      await result.current.startUpdate('my-plugin');
    });
    await act(async () => {
      await result.current.updateNoticeProps.onAgree();
    });

    expect(mockState.install).not.toHaveBeenCalled();
    const pending = result.current.dialogProps.pendingInstall;
    expect(pending?.kind).toBe('github');
    expect(pending?.selectedOverwriteNames).toEqual(['mine']);
    expect(pending?.approveHooks).toBe(true);
  });
});
