import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SkillImportPreview } from '@/types/skills';
import { useSessionPluginInstall } from '../useSessionPluginInstall';

const mockState = vi.hoisted(() => ({
  preview: vi.fn(),
  install: vi.fn(),
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
}));

const URL = 'https://github.com/o/r';

function preview(overrides: Partial<SkillImportPreview>): SkillImportPreview {
  return {
    kind: 'skillBundle',
    discoveredSkills: [{ name: 'alpha', description: 'a' }],
    conflicts: [],
    hooks: [],
    hooksRequireApproval: false,
    commands: [],
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
  });

  it('installs immediately when there is nothing to approve', async () => {
    mockState.preview.mockResolvedValue(preview({}));
    const { result } = renderHook(() => useSessionPluginInstall('s1'));

    let consumed = false;
    await act(async () => {
      consumed = await result.current.startInstall(URL);
    });

    expect(consumed).toBe(true);
    expect(mockState.install).toHaveBeenCalledWith('s1', URL, false, [], false);
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

    expect(mockState.install).toHaveBeenCalledWith('s1', URL, false, [], true);
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
});
