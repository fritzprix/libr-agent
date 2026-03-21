import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MentionTextarea } from '../MentionTextarea';

const mockUseScopedSkills = vi.fn();
const mockUseWorkspaceFiles = vi.fn();
const mockUseInputToken = vi.fn();
const mockUsePlaybookSearch = vi.fn();
let dropdownMode:
  | { kind: 'types' | 'skills' | 'files' | 'tools' | 'playbooks'; items: unknown[] }
  | null = null;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('@/features/agent/hooks/useScopedSkills', () => ({
  useScopedSkills: (...args: unknown[]) => mockUseScopedSkills(...args),
}));

vi.mock('@/features/agent/hooks/useWorkspaceFiles', () => ({
  useWorkspaceFiles: (...args: unknown[]) => mockUseWorkspaceFiles(...args),
}));

vi.mock('@/features/agent/hooks/useInputToken', () => ({
  useInputToken: (...args: unknown[]) => mockUseInputToken(...args),
}));

vi.mock('@/features/agent/hooks/usePlaybookSearch', () => ({
  usePlaybookSearch: (...args: unknown[]) => mockUsePlaybookSearch(...args),
}));

vi.mock('@/features/agent/components/InputTokenDropdown', () => ({
  InputTokenDropdown: ({
    mode,
  }: {
    mode: {
      kind: 'types' | 'skills' | 'files' | 'tools' | 'playbooks';
      items: unknown[];
    };
  }) => {
    dropdownMode = mode;
    return <div data-testid="input-token-dropdown" />;
  },
}));

vi.mock('@/components/ui/textarea', () => ({
  Textarea: ({
    value,
    placeholder,
  }: {
    value: string;
    placeholder?: string;
  }) => <textarea value={value} placeholder={placeholder} readOnly />,
}));

describe('MentionTextarea', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dropdownMode = null;

    mockUseScopedSkills.mockReturnValue({ skills: [], isLoading: false });
    mockUseWorkspaceFiles.mockReturnValue([]);
    mockUsePlaybookSearch.mockReturnValue([]);
    mockUseInputToken.mockReturnValue({
      stage: { kind: 'idle' },
      typeResults: [],
      skillResults: [],
      fileResults: [],
      onInputChange: vi.fn(),
      onTypeSelect: vi.fn(),
      onArgSelect: vi.fn(),
      onDismiss: vi.fn(),
    });
  });

  it('uses workspace-scoped file suggestions for @file autocomplete', () => {
    mockUseInputToken.mockReturnValue({
      stage: { kind: 'typing-arg', typeName: 'file', query: 'src' },
      typeResults: [],
      skillResults: [],
      fileResults: [],
      onInputChange: vi.fn(),
      onTypeSelect: vi.fn(),
      onArgSelect: vi.fn(),
      onDismiss: vi.fn(),
    });
    mockUseWorkspaceFiles.mockReturnValue(['src/main.ts']);

    render(
      <MentionTextarea
        value="@file:src"
        onChange={vi.fn()}
        workspacePath="/tmp/workspace"
      />,
    );

    expect(mockUseWorkspaceFiles).toHaveBeenCalledWith(
      undefined,
      'src',
      '/tmp/workspace',
    );
    expect(screen.getByTestId('input-token-dropdown')).toBeInTheDocument();
    expect(dropdownMode).toEqual({
      kind: 'files',
      items: ['src/main.ts'],
    });
  });
});
