import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { OrgCard } from '../OrgCard';
import type { OrgSummary } from '../org-sessions';
import { toast } from 'sonner';
import type { AgentSession } from '@/models/agent';

const mockNavigate = vi.fn();
const mockDeleteSession = vi.fn();
const mockOnDeleted = vi.fn();

vi.mock('react-router-dom', () => ({
  useNavigate: () => mockNavigate,
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    deleteSession: mockDeleteSession,
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultString?: string) => defaultString || key,
  }),
}));

// Mock ResizeObserver which is needed by some Radix components
global.ResizeObserver = vi.fn().mockImplementation(() => ({
  observe: vi.fn(),
  unobserve: vi.fn(),
  disconnect: vi.fn(),
}));

const mockOrg: OrgSummary = {
  orgId: 'org-1',
  orgName: 'Research Org',
  orgRootSessionId: 'root-1',
  rootSession: {
    id: 'root-1',
    name: 'Root Session',
    status: 'idle',
    model: 'gpt-4',
    provider: 'openai',
    createdAt: new Date('2026-04-05T00:00:00Z'),
    updatedAt: new Date('2026-04-05T01:00:00Z'),
    orgId: 'org-1',
    orgName: 'Research Org',
    orgRootSessionId: 'root-1',
    yoloMode: false,
  } as unknown as AgentSession,
  members: [],
  memberCount: 5,
  busyCount: 1,
  updatedAt: new Date('2026-04-05T01:00:00Z'),
};

describe('OrgCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockOnDeleted.mockResolvedValue(undefined);
  });

  it('renders organization information correctly', () => {
    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    expect(screen.getByText('Research Org')).toBeInTheDocument();
    // Use getAllByText because 'Root Session' appears in the title and the value
    expect(screen.getAllByText('Root Session').length).toBeGreaterThan(0);
    expect(screen.getByText('5')).toBeInTheDocument(); // Member count
  });

  it('navigates to root session when resume button is clicked', () => {
    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    fireEvent.click(screen.getByText('Resume Root Session'));
    expect(mockNavigate).toHaveBeenCalledWith('/agent/root-1');
  });

  it('opens confirmation dialog and handles successful deletion', async () => {
    mockDeleteSession.mockResolvedValueOnce(undefined);
    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    // Click trash icon
    const deleteButton = screen.getByRole('button', { name: 'Delete Organization' });
    fireEvent.click(deleteButton);

    // Dialog should be open
    expect(screen.getByText('Delete Organization?')).toBeInTheDocument();

    // Click confirm delete - the button text is 'common.delete' or 'Delete' depending on mock
    const confirmButton = screen.getByRole('button', { name: 'Delete' });
    fireEvent.click(confirmButton);

    expect(mockDeleteSession).toHaveBeenCalledWith('root-1');

    await waitFor(() => {
      expect(toast.success).toHaveBeenCalledWith('Organization deleted');
      expect(mockOnDeleted).toHaveBeenCalledTimes(1);
      expect(screen.queryByText('Delete Organization?')).not.toBeInTheDocument();
    });
  });

  it('handles deletion failure and keeps dialog open', async () => {
    mockDeleteSession.mockRejectedValueOnce(new Error('Delete failed'));
    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Organization' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalledWith('Failed to delete organization');
      expect(mockOnDeleted).not.toHaveBeenCalled();
      // Dialog should still be open
      expect(screen.getByText('Delete Organization?')).toBeInTheDocument();
    });
  });

  it('keeps the success path when the post-delete refresh fails', async () => {
    mockDeleteSession.mockResolvedValueOnce(undefined);
    mockOnDeleted.mockRejectedValueOnce(new Error('Refresh failed'));

    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Organization' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    await waitFor(() => {
      expect(toast.success).toHaveBeenCalledWith('Organization deleted');
      expect(toast.error).not.toHaveBeenCalled();
      expect(mockOnDeleted).toHaveBeenCalledTimes(1);
      expect(screen.queryByText('Delete Organization?')).not.toBeInTheDocument();
    });
  });

  it('disables buttons while deleting', async () => {
    // Mock a slow deletion
    let resolveDelete: (value: void | PromiseLike<void>) => void;
    const deletePromise = new Promise<void>((resolve) => {
      resolveDelete = resolve;
    });
    mockDeleteSession.mockReturnValue(deletePromise);

    render(<OrgCard org={mockOrg} onDeleted={mockOnDeleted} />);

    fireEvent.click(screen.getByRole('button', { name: 'Delete Organization' }));
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));

    // Buttons should be disabled
    // Note: AlertDialogCancel uses t('common.cancel') which returns 'common.cancel' in our mock
    expect(screen.getByRole('button', { name: 'common.cancel' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDisabled();

    // Complete the deletion
    resolveDelete!(undefined);

    await waitFor(() => {
      expect(screen.queryByText('Delete Organization?')).not.toBeInTheDocument();
    });
  });
});
