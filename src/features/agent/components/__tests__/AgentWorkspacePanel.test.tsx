import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AgentWorkspacePanel } from '../AgentWorkspacePanel';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom';
import { open } from '@tauri-apps/plugin-dialog';

// Mock dependencies
vi.mock('@/hooks/use-rust-backend', () => {
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
  const mockActions = {
    submit: vi.fn(),
    injectMessages: vi.fn(),
  };
  const mockState = {
    messages: [],
  };
  return {
    useAgentChatActions: () => mockActions,
    useAgentChatState: () => mockState,
  };
});

vi.mock('@/context/DnDContext', () => {
  const mockDnD = {
    subscribe: vi.fn(() => vi.fn()),
  };
  return {
    useDnDContext: () => mockDnD,
  };
});

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
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
  });

  it('renders accessibility labels correctly', async () => {
    render(<AgentWorkspacePanel />);

    // Wait for initial load
    await waitFor(() => {
      expect(screen.getByText('agent.workspace.title')).toBeInTheDocument();
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
      expect(screen.getByText('agent.workspace.title')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    fireEvent.click(uploadZone);

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });

  it('triggers file upload dialog on Enter key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getByText('agent.workspace.title')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    fireEvent.keyDown(uploadZone, { key: 'Enter' });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });

  it('triggers file upload dialog on Space key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getByText('agent.workspace.title')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('agent.workspace.uploadAria');
    fireEvent.keyDown(uploadZone, { key: ' ' });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'agent.workspace.selectFilesTitle',
    });
  });
});
