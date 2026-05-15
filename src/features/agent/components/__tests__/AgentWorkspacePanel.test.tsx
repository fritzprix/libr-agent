import { act, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AgentWorkspacePanel } from '../AgentWorkspacePanel';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  DragAndDropEvent,
  DragAndDropPayload,
} from '@/context/DnDContext';
import * as backend from '@/lib/backend';
import { toast } from 'sonner';
import * as pathApi from '@tauri-apps/api/path';

let latestHandler:
  | ((event: DragAndDropEvent, payload: DragAndDropPayload) => void)
  | undefined;
const mocks = vi.hoisted(() => ({
  subscribe: vi.fn(
    (
      _ref: unknown,
      handler: (event: DragAndDropEvent, payload: DragAndDropPayload) => void,
    ) => {
      latestHandler = handler;
      return vi.fn();
    },
  ),
}));
const mockRustBackend = {
  listWorkspaceFiles: vi.fn().mockResolvedValue([]),
  openWorkspaceFileWithDefaultApp: vi.fn(),
  agentCallBuiltinTool: vi.fn(),
  getWorkspaceOverride: vi.fn().mockResolvedValue(''),
  setWorkspaceOverride: vi.fn(),
  cancelWorkspaceOverride: vi.fn(),
  openWorkspaceInExplorer: vi.fn(),
  openWorkspaceInTerminal: vi.fn(),
};
const mockChatActions = {
  submit: vi.fn(),
  injectMessages: vi.fn(),
};

// Mock dependencies
vi.mock('@/hooks/use-rust-backend', () => {
  return {
    useRustBackend: () => mockRustBackend,
  };
});

vi.mock('@/lib/backend', () => ({
  openWorkspaceInExplorer: vi.fn(),
  openWorkspaceInTerminal: vi.fn(),
  getWorkspaceOverride: vi.fn().mockResolvedValue(''),
  setWorkspaceOverride: vi.fn(),
  cancelWorkspaceOverride: vi.fn(),
  checkDroppedPathType: vi.fn(),
  registerDroppedFiles: vi.fn(),
}));

vi.mock('@/context/AgentSessionContext', () => {
  const mockState = {
    session: { id: 'session-123' },
  };
  return {
    useAgentSessionState: () => mockState,
  };
});

vi.mock('@/context/AgentChatContext', () => {
  const mockState = {
    messages: [],
  };
  return {
    useAgentChatActions: () => mockChatActions,
    useAgentChatState: () => mockState,
  };
});

vi.mock('@/context/DnDContext', () => {
  const mockDnD = {
    subscribe: mocks.subscribe,
  };
  return {
    useDnDContext: () => mockDnD,
  };
});

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

vi.mock('@tauri-apps/api/path', () => ({
  join: vi.fn(),
}));

// Mock sonner toast
vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('AgentWorkspacePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    latestHandler = undefined;
    mockRustBackend.agentCallBuiltinTool.mockResolvedValue({
      content: [{ type: 'text', text: 'Imported files successfully' }],
      isError: false,
    });
    vi.mocked(pathApi.join).mockImplementation(
      async (basePath: string, childPath: string) => {
        if (basePath === './') {
          return `./${childPath}`;
        }
        return `${basePath}/${childPath}`;
      },
    );
  });

  it('skips DnD subscription while hidden', async () => {
    render(<AgentWorkspacePanel isVisible={false} />);

    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    expect(mocks.subscribe).not.toHaveBeenCalled();
  });

  it('renders accessibility labels correctly', async () => {
    render(<AgentWorkspacePanel />);

    // Wait for initial load
    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    // Check buttons by aria-label
    expect(screen.getByLabelText('agent.workspace.openInExplorerAria')).toBeInTheDocument();
    expect(screen.getByLabelText('agent.workspace.openInTerminalAria')).toBeInTheDocument();
    expect(screen.getByLabelText('agent.workspace.refreshAria')).toBeInTheDocument();

    // Check input by aria-label
    expect(screen.getByLabelText('agent.workspace.overrideAria')).toBeInTheDocument();

    // Check upload zone by aria-label
    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    expect(uploadZone).toBeInTheDocument();
    expect(uploadZone).toHaveAttribute('role', 'button');
    expect(uploadZone).toHaveAttribute('tabIndex', '0');
  });

  it('triggers file upload dialog on click', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    await act(async () => {
      fireEvent.click(uploadZone);
    });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });

  it('triggers file upload dialog on Enter key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    await act(async () => {
      fireEvent.keyDown(uploadZone, { key: 'Enter' });
    });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });

  it('triggers file upload dialog on Space key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    await act(async () => {
      fireEvent.keyDown(uploadZone, { key: ' ' });
    });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });

  it('sets workspace override directly for a dropped directory', async () => {
    vi.mocked(backend.registerDroppedFiles).mockResolvedValue();
    vi.mocked(backend.checkDroppedPathType).mockResolvedValue('directory');

    render(<AgentWorkspacePanel />);

    await act(async () => {
      latestHandler?.('drop', { paths: ['C:\\workspace'] });
    });

    await waitFor(() => {
      expect(backend.setWorkspaceOverride).toHaveBeenCalledWith(
        'session-123',
        'C:\\workspace',
      );
    });

    expect(backend.registerDroppedFiles).toHaveBeenCalledWith(['C:\\workspace']);
  });

  it('keeps dropped files on the existing import flow', async () => {
    vi.mocked(backend.registerDroppedFiles).mockResolvedValue();
    vi.mocked(backend.checkDroppedPathType).mockResolvedValue('file');

    render(<AgentWorkspacePanel />);

    await act(async () => {
      latestHandler?.('drop', { paths: ['C:\\workspace\\notes.md'] });
    });

    await waitFor(() => {
      expect(mockRustBackend.agentCallBuiltinTool).toHaveBeenCalledWith(
        'session-123',
        'workspace__importFiles',
        expect.objectContaining({
          files: [
            expect.objectContaining({
              srcAbsPath: 'C:\\workspace\\notes.md',
              destRelPath: 'notes.md',
            }),
          ],
        }),
      );
    });

    expect(backend.setWorkspaceOverride).not.toHaveBeenCalled();
  });

  it('batch imports multiple dropped files without changing workspace override', async () => {
    vi.mocked(backend.registerDroppedFiles).mockResolvedValue();
    vi.mocked(backend.checkDroppedPathType).mockResolvedValue('file');

    render(<AgentWorkspacePanel />);

    await act(async () => {
      latestHandler?.('drop', {
        paths: ['C:\\workspace\\notes.md', 'C:\\workspace\\todo.txt'],
      });
    });

    await waitFor(() => {
      expect(mockRustBackend.agentCallBuiltinTool).toHaveBeenCalledWith(
        'session-123',
        'workspace__importFiles',
        {
          files: [
            {
              srcAbsPath: 'C:\\workspace\\notes.md',
              destRelPath: 'notes.md',
            },
            {
              srcAbsPath: 'C:\\workspace\\todo.txt',
              destRelPath: 'todo.txt',
            },
          ],
        },
      );
    });

    expect(backend.registerDroppedFiles).toHaveBeenCalledWith([
      'C:\\workspace\\notes.md',
      'C:\\workspace\\todo.txt',
    ]);
    expect(backend.setWorkspaceOverride).not.toHaveBeenCalled();
  });

  it('rejects mixed file and folder drops', async () => {
    vi.mocked(backend.registerDroppedFiles).mockResolvedValue();
    vi.mocked(backend.checkDroppedPathType)
      .mockResolvedValueOnce('file')
      .mockResolvedValueOnce('directory');

    render(<AgentWorkspacePanel />);

    await act(async () => {
      latestHandler?.('drop', {
        paths: ['C:\\workspace\\notes.md', 'C:\\workspace-folder'],
      });
    });

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'agent.workspace.dropMixedFoldersError',
      );
    });

    expect(backend.setWorkspaceOverride).not.toHaveBeenCalled();
    expect(mockRustBackend.agentCallBuiltinTool).not.toHaveBeenCalled();
  });

  it('rejects dropping multiple folders at once', async () => {
    vi.mocked(backend.registerDroppedFiles).mockResolvedValue();
    vi.mocked(backend.checkDroppedPathType)
      .mockResolvedValueOnce('directory')
      .mockResolvedValueOnce('directory');

    render(<AgentWorkspacePanel />);

    await act(async () => {
      latestHandler?.('drop', {
        paths: ['C:\\workspace-a', 'C:\\workspace-b'],
      });
    });

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith(
        'agent.workspace.dropMixedFoldersError',
      );
    });

    expect(backend.setWorkspaceOverride).not.toHaveBeenCalled();
  });

  it('provides accessible focus targets for disabled action buttons during native opening', async () => {
    // Keep the native opening pending
    let resolveOpening: () => void;
    const openingPromise = new Promise<void>((resolve) => {
      resolveOpening = resolve;
    });
    vi.mocked(backend.openWorkspaceInExplorer).mockReturnValue(openingPromise);

    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getAllByText('agent.workspace.title').length).toBeGreaterThan(
        0,
      );
    });

    const explorerButton = screen.getByLabelText(
      'agent.workspace.openInExplorerAria',
    );

    // Trigger the opening
    await act(async () => {
      fireEvent.click(explorerButton);
    });

    // Button should be disabled
    expect(explorerButton).toBeDisabled();

    // The wrapper span should now be focusable and have accessibility labels
    // We look for role="button" and the specific aria-label on the span
    const wrappers = screen.getAllByRole('button', {
      name: 'agent.workspace.openInExplorerAria',
    });
    // One is the disabled button, the other is the focusable span wrapper
    const focusableWrapper = wrappers.find(
      (el) => el.tagName.toLowerCase() === 'span',
    );

    expect(focusableWrapper).toBeInTheDocument();
    expect(focusableWrapper).toHaveAttribute('tabIndex', '0');
    expect(focusableWrapper).toHaveAttribute('aria-disabled', 'true');

    // Clean up
    await act(async () => {
      resolveOpening!();
    });
  });
});
