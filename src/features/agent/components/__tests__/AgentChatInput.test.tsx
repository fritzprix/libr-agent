import { createEvent, fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentChatInput } from '../AgentChatInput';
import { AGENT_ATTACHMENT_PICKER_ACCEPT } from '@/features/agent/lib/attachment-picker';

const fileAttachmentProps = vi.hoisted(
  () =>
    ({
      current: null as null | Record<string, unknown>,
    }) as { current: null | Record<string, unknown> },
);

const mocks = vi.hoisted(() => ({
  attachFiles: vi.fn(),
  handleFileAttachment: vi.fn(),
  removeFile: vi.fn(),
  processFileDrop: vi.fn(),
  validateFiles: vi.fn(() => true),
  commitPendingFiles: vi.fn(),
  clearPendingFiles: vi.fn(),
  refetchSessionFiles: vi.fn(),
  cancel: vi.fn(),
  resume: vi.fn(),
  submit: vi.fn(),
  refreshSkills: vi.fn(),
  setInput: vi.fn(),
  onTokenInputChange: vi.fn(),
  handleSubmit: vi.fn((e?: Event) => e?.preventDefault?.()),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: {
      id: 'session-1',
      assistant: {
        id: 'assistant-1',
      },
    },
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    submit: mocks.submit,
    isSessionLoading: false,
    workflowStatus: 'idle',
    cancel: mocks.cancel,
    resume: mocks.resume,
  }),
}));

vi.mock('@/context/LLMServiceContext', () => ({
  useLLMService: () => ({
    isCompacting: () => false,
  }),
}));

vi.mock('@/context/DnDContext', () => ({
  useDnDContext: () => ({
    subscribe: () => () => undefined,
  }),
}));

vi.mock('@/features/agent/hooks/useScopedSkills', () => ({
  useScopedSkills: () => ({
    skills: [],
    refresh: mocks.refreshSkills,
  }),
}));

vi.mock('@/hooks/use-agent-tools', () => ({
  useAgentTools: () => ({
    availableTools: [],
  }),
}));

vi.mock('@/features/agent/hooks/useWorkspaceFiles', () => ({
  useWorkspaceFiles: () => [],
}));

vi.mock('@/features/agent/hooks/usePlaybookSearch', () => ({
  usePlaybookSearch: () => [],
}));

vi.mock('@/features/agent/hooks/useInputToken', () => ({
  useInputToken: () => ({
    stage: { kind: 'idle' },
    typeResults: [],
    skillResults: [],
    toolResults: [],
    onInputChange: mocks.onTokenInputChange,
    onTypeSelect: vi.fn(),
    onArgSelect: vi.fn(),
    onDismiss: vi.fn(),
  }),
}));

vi.mock('@/hooks/useTextareaAutosize', () => ({
  useTextareaAutosize: vi.fn(),
}));

vi.mock('@/features/agent/hooks/useAgentFileAttachment', () => ({
  useAgentFileAttachment: () => ({
    pendingFiles: [],
    commitPendingFiles: mocks.commitPendingFiles,
    clearPendingFiles: mocks.clearPendingFiles,
    isAttachmentLoading: false,
    attachFiles: mocks.attachFiles,
    handleFileAttachment: mocks.handleFileAttachment,
    removeFile: mocks.removeFile,
    processFileDrop: mocks.processFileDrop,
    validateFiles: mocks.validateFiles,
    refetchSessionFiles: mocks.refetchSessionFiles,
  }),
}));

vi.mock('@/features/agent/hooks/useChatSubmit', () => ({
  useChatSubmit: () => ({
    input: '',
    setInput: mocks.setInput,
    isSubmitting: false,
    handleSubmit: mocks.handleSubmit,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/components/ui', () => ({
  Button: ({
    children,
    ...props
  }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
  FileAttachment: (props: Record<string, unknown>) => {
    fileAttachmentProps.current = props;
    return <div data-testid="file-attachment" />;
  },
  Tooltip: ({ children }: { children: ReactNode }) => <>{children}</>,
  TooltipTrigger: ({ children }: { children: ReactNode; asChild?: boolean }) => (
    <>{children}</>
  ),
  TooltipContent: ({ children }: { children: ReactNode }) => <>{children}</>,
}));

describe('AgentChatInput', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    fileAttachmentProps.current = null;
  });

  it('passes the explicit agent attachment accept policy to the picker', () => {
    render(<AgentChatInput />);

    expect(fileAttachmentProps.current).toMatchObject({
      accept: AGENT_ATTACHMENT_PICKER_ACCEPT,
    });
  });

  it('attaches pasted images from clipboard items', () => {
    render(<AgentChatInput />);

    const textarea = screen.getByLabelText('agent.input.ariaLabel');
    const imageFile = new File(['image-bytes'], 'clipboard.png', {
      type: 'image/png',
    });
    const pasteEvent = createEvent.paste(textarea);

    Object.defineProperty(pasteEvent, 'clipboardData', {
      value: {
        items: [
          {
            kind: 'file',
            type: 'image/png',
            getAsFile: () => imageFile,
          },
        ],
        files: [],
        getData: vi.fn(() => ''),
      },
    });

    fireEvent(textarea, pasteEvent);

    expect(pasteEvent.defaultPrevented).toBe(true);
    expect(mocks.attachFiles).toHaveBeenCalledWith([imageFile]);
  });

  it('preserves pasted text while attaching clipboard images', () => {
    render(<AgentChatInput />);

    const textarea = screen.getByLabelText('agent.input.ariaLabel');
    const imageFile = new File(['image-bytes'], 'clipboard.png', {
      type: 'image/png',
    });
    const pasteEvent = createEvent.paste(textarea);

    Object.defineProperty(pasteEvent, 'clipboardData', {
      value: {
        items: [
          {
            kind: 'file',
            type: 'image/png',
            getAsFile: () => imageFile,
          },
        ],
        files: [],
        getData: vi.fn((type: string) =>
          type === 'text/plain' ? 'pasted text' : '',
        ),
      },
    });

    fireEvent(textarea, pasteEvent);

    expect(mocks.setInput).toHaveBeenCalledWith('pasted text');
    expect(mocks.onTokenInputChange).toHaveBeenCalledWith('pasted text', 11);
    expect(mocks.attachFiles).toHaveBeenCalledWith([imageFile]);
  });

  it('ignores non-image paste payloads', () => {
    render(<AgentChatInput />);

    const textarea = screen.getByLabelText('agent.input.ariaLabel');
    const pasteEvent = createEvent.paste(textarea);

    Object.defineProperty(pasteEvent, 'clipboardData', {
      value: {
        items: [
          {
            kind: 'string',
            type: 'text/plain',
            getAsFile: () => null,
          },
        ],
        files: [],
        getData: vi.fn(() => 'plain text'),
      },
    });

    fireEvent(textarea, pasteEvent);

    expect(pasteEvent.defaultPrevented).toBe(false);
    expect(mocks.attachFiles).not.toHaveBeenCalled();
    expect(mocks.setInput).not.toHaveBeenCalled();
  });

  it('falls back to clipboard files when items are unavailable', () => {
    render(<AgentChatInput />);

    const textarea = screen.getByLabelText('agent.input.ariaLabel');
    const imageFile = new File(['image-bytes'], 'clipboard.png', {
      type: 'image/png',
    });
    const pasteEvent = createEvent.paste(textarea);

    Object.defineProperty(pasteEvent, 'clipboardData', {
      value: {
        items: [],
        files: [imageFile],
        getData: vi.fn(() => ''),
      },
    });

    fireEvent(textarea, pasteEvent);

    expect(mocks.attachFiles).toHaveBeenCalledWith([imageFile]);
  });
});
