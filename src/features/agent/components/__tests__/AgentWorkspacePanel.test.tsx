import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { AgentWorkspacePanel } from '../AgentWorkspacePanel';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom';
import { open } from '@tauri-apps/plugin-dialog';

// Mock dependencies
vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    listWorkspaceFiles: vi.fn().mockResolvedValue([]),
    openWorkspaceFileWithDefaultApp: vi.fn(),
    agentCallBuiltinTool: vi.fn(),
    getWorkspaceOverride: vi.fn().mockResolvedValue(''),
    setWorkspaceOverride: vi.fn(),
    cancelWorkspaceOverride: vi.fn(),
    openWorkspaceInExplorer: vi.fn(),
    openWorkspaceInTerminal: vi.fn(),
  }),
}));

vi.mock('@/lib/backend', () => ({
  openWorkspaceInExplorer: vi.fn(),
  openWorkspaceInTerminal: vi.fn(),
  getWorkspaceOverride: vi.fn().mockResolvedValue(''),
  setWorkspaceOverride: vi.fn(),
  cancelWorkspaceOverride: vi.fn(),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'session-123' },
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChatActions: () => ({
    submit: vi.fn(),
    injectMessages: vi.fn(),
  }),
  useAgentChatState: () => ({
    messages: [],
  }),
}));

vi.mock('@/context/DnDContext', () => ({
  useDnDContext: () => ({
    subscribe: vi.fn(() => vi.fn()),
  }),
}));

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

describe('AgentWorkspacePanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders accessibility labels correctly', async () => {
    render(<AgentWorkspacePanel />);

    // Wait for initial load
    await waitFor(() => {
        expect(screen.getByText('Workspace Files')).toBeInTheDocument();
    });

    // Check buttons by aria-label
    expect(screen.getByLabelText('Open in Explorer')).toBeInTheDocument();
    expect(screen.getByLabelText('Open in Terminal')).toBeInTheDocument();
    expect(screen.getByLabelText('Refresh workspace files')).toBeInTheDocument();

    // Check input by aria-label
    expect(screen.getByLabelText('Workspace override path')).toBeInTheDocument();

    // Check upload zone by aria-label
    const uploadZone = screen.getByLabelText('Upload files to workspace');
    expect(uploadZone).toBeInTheDocument();
    expect(uploadZone).toHaveAttribute('role', 'button');
    expect(uploadZone).toHaveAttribute('tabIndex', '0');
  });

  it('triggers file upload dialog on click', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getByText('Workspace Files')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('Upload files to workspace');
    fireEvent.click(uploadZone);

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'Select files to upload',
    });
  });

  it('triggers file upload dialog on Enter key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getByText('Workspace Files')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('Upload files to workspace');
    fireEvent.keyDown(uploadZone, { key: 'Enter' });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'Select files to upload',
    });
  });

  it('triggers file upload dialog on Space key', async () => {
    render(<AgentWorkspacePanel />);

    await waitFor(() => {
      expect(screen.getByText('Workspace Files')).toBeInTheDocument();
    });

    const uploadZone = screen.getByLabelText('Upload files to workspace');
    fireEvent.keyDown(uploadZone, { key: ' ' });

    expect(vi.mocked(open)).toHaveBeenCalledWith({
      multiple: true,
      title: 'Select files to upload',
    });
  });
});
