import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, type Mock } from 'vitest';
import { AgentChatAttachedFiles } from '../AgentChatAttachedFiles';
import * as useAgentResourceAttachmentModule from '@/features/agent/hooks/useAgentResourceAttachment';

// Mock the hook
vi.mock('@/features/agent/hooks/useAgentResourceAttachment', () => ({
  useAgentResourceAttachment: vi.fn(),
}));

describe('AgentChatAttachedFiles', () => {
  const mockRemoveFile = vi.fn();

  it('renders attached files', () => {
    (useAgentResourceAttachmentModule.useAgentResourceAttachment as Mock).mockReturnValue({
      pendingFiles: [
        { contentId: '1', filename: 'test.txt' },
        { contentId: '2', filename: 'image.png' },
      ],
      removeFile: mockRemoveFile,
    });

    render(<AgentChatAttachedFiles />);

    expect(screen.getByText('test.txt')).toBeInTheDocument();
    expect(screen.getByText('image.png')).toBeInTheDocument();
  });

  it('buttons should have accessible labels', () => {
    (useAgentResourceAttachmentModule.useAgentResourceAttachment as Mock).mockReturnValue({
      pendingFiles: [
        { contentId: '1', filename: 'test.txt' },
      ],
      removeFile: mockRemoveFile,
    });

    render(<AgentChatAttachedFiles />);

    expect(screen.getByRole('button', { name: 'Remove test.txt' })).toBeInTheDocument();
  });
});
