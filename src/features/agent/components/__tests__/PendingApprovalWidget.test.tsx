import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';

import { PendingApprovalWidget } from '../PendingApprovalWidget';
import type { PendingApproval } from '@/context/AgentSessionContext';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue ?? key,
  }),
}));

const standardApproval: PendingApproval = {
  toolCallId: 'call-standard',
  toolName: 'readFile',
  arguments: '{"path":"README.md"}',
  approvalKind: 'standard',
  requestId: 'req-standard',
  description: 'Standard approval',
  inputPreview: 'README.md',
};

const hardApproval: PendingApproval = {
  toolCallId: 'call-hard',
  toolName: 'runShell',
  arguments: '{"command":"rm -rf /important"}',
  approvalKind: 'hard',
  requestId: 'req-hard',
  description: 'Hard approval',
  inputPreview: 'rm -rf /important',
};

describe('PendingApprovalWidget', () => {
  it('surfaces hard approvals distinctly when YOLO mode is enabled', () => {
    render(
      <PendingApprovalWidget
        approvals={[hardApproval]}
        executionMode="yolo"
        onRespond={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText('Hard Approval Required')).toBeInTheDocument();
    expect(screen.getByText('Hard approval')).toBeInTheDocument();
    expect(
      screen.getByText(
        'YOLO mode is on, but this high-risk action still requires manual approval.',
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Approve high-risk action' }),
    ).toBeInTheDocument();
  });

  it('approves the priority approval on Enter', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <PendingApprovalWidget
        approvals={[standardApproval, hardApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Enter' });

    await waitFor(() => {
      expect(onRespond).toHaveBeenCalledWith('call-hard', true);
    });
  });

  it('rejects the priority approval on Escape', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <PendingApprovalWidget
        approvals={[standardApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => {
      expect(onRespond).toHaveBeenCalledWith('call-standard', false);
    });
  });

  it('does not respond when another Escape consumer already prevented the event', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);
    const preventEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
      }
    };

    window.addEventListener('keydown', preventEscape);
    render(
      <PendingApprovalWidget
        approvals={[standardApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Escape' });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onRespond).not.toHaveBeenCalled();
    window.removeEventListener('keydown', preventEscape);
  });

  it('ignores Enter while typing in an input', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <>
        <input aria-label="chat-input" />
        <PendingApprovalWidget
          approvals={[standardApproval]}
          executionMode="normal"
          onRespond={onRespond}
        />
      </>,
    );

    const input = screen.getByLabelText('chat-input');
    input.focus();
    fireEvent.keyDown(input, { key: 'Enter' });

    expect(onRespond).not.toHaveBeenCalled();
  });
});
