import { createRef } from 'react';
import { render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AgentDraftChatView from '../AgentDraftChatView';

const mockUseAgentDraftChat = vi.fn();
const mockUseWorkspaceFiles = vi.fn();

let dropdownMode:
  | { kind: 'types' | 'skills' | 'files' | 'tools' | 'playbooks'; items: unknown[] }
  | null = null;

const createDraftChatState = () => ({
  assistant: {
    id: 'assistant-1',
    name: 'Draft Assistant',
    description: 'Draft description',
    allowedBuiltInServiceAliases: [],
  },
  isLoadingAssistant: false,
  input: '@file:src',
  setInput: vi.fn(),
  isSubmitting: false,
  overrideModel: undefined,
  setOverrideModel: vi.fn(),
  overrideProvider: undefined,
  setOverrideProvider: vi.fn(),
  builtinServices: [],
  mcpServers: [],
  pendingFiles: [],
  workspaceOverride: 'C:\\workspace',
  setWorkspaceOverride: vi.fn(),
  dragState: 'none',
  profileDragState: 'none',
  isAttachmentLoading: false,
  formRef: createRef<HTMLFormElement>(),
  profileAreaRef: createRef<HTMLDivElement>(),
  textareaRef: createRef<HTMLTextAreaElement>(),
  handleFileAdd: vi.fn(),
  handleFileRemove: vi.fn(),
  handleSubmit: vi.fn(),
  stage: { kind: 'typing-arg', typeName: 'file', query: 'src' } as const,
  typeResults: [],
  skillResults: [],
  onInputChange: vi.fn(),
  onTypeSelect: vi.fn(
    (typeName: string, currentValue: string) => currentValue + typeName,
  ),
  onArgSelect: vi.fn(
    (arg: string, currentValue: string) => `${currentValue}${arg}`,
  ),
  onDismiss: vi.fn(),
});

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('@/context/SettingsContext', () => ({
  useSettings: () => ({
    value: {
      advanced: {
        defaultMaxOutputTokens: 8192,
      },
    },
  }),
}));

vi.mock('../hooks/useAgentDraftChat', () => ({
  useAgentDraftChat: () => mockUseAgentDraftChat(),
}));

vi.mock('../hooks/useWorkspaceFiles', () => ({
  useWorkspaceFiles: (...args: unknown[]) => mockUseWorkspaceFiles(...args),
}));

vi.mock('../components/AgentModelPicker', () => ({
  AgentModelPicker: () => <div data-testid="agent-model-picker" />,
}));

vi.mock('../components/InputTokenDropdown', () => ({
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

vi.mock('../components/AgentDraftWorkspacePreviewPanel', () => ({
  AgentDraftWorkspacePreviewPanel: () => (
    <div data-testid="draft-workspace-preview" />
  ),
}));

describe('AgentDraftChatView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dropdownMode = null;

    mockUseWorkspaceFiles.mockReturnValue([]);
    mockUseAgentDraftChat.mockReturnValue(createDraftChatState());
  });

  it('renders the draft workspace preview when workspace override is set', () => {
    render(
      <MemoryRouter>
        <AgentDraftChatView />
      </MemoryRouter>,
    );

    expect(screen.getByTestId('draft-workspace-preview')).toBeInTheDocument();
  });

  it('does not render the draft workspace preview without workspace override', () => {
    mockUseAgentDraftChat.mockReturnValue({
      ...createDraftChatState(),
      workspaceOverride: null,
      stage: { kind: 'idle' },
      input: '',
    });

    render(
      <MemoryRouter>
        <AgentDraftChatView />
      </MemoryRouter>,
    );

    expect(
      screen.queryByTestId('draft-workspace-preview'),
    ).not.toBeInTheDocument();
  });

  it('uses workspace-scoped file suggestions for draft @file autocomplete', () => {
    mockUseWorkspaceFiles.mockReturnValue(['src/main.ts']);

    render(
      <MemoryRouter>
        <AgentDraftChatView />
      </MemoryRouter>,
    );

    expect(mockUseWorkspaceFiles).toHaveBeenCalledWith(
      undefined,
      'src',
      'C:\\workspace',
    );
    expect(screen.getByTestId('input-token-dropdown')).toBeInTheDocument();
    expect(dropdownMode).toEqual({
      kind: 'files',
      items: ['src/main.ts'],
    });
  });
});
