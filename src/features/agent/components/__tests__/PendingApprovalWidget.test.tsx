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

const rootApproval: PendingApproval = {
  toolCallId: 'call-root',
  toolName: 'workspace__runShell',
  arguments: '{"command":"apt-get install -y nodejs","asRoot":true}',
  approvalKind: 'root',
  requestId: 'req-root',
  description: 'Run as root: apt-get install -y nodejs',
  inputPreview: 'apt-get install -y nodejs',
};

describe('PendingApprovalWidget', () => {
  it('shows that a root approval runs as root, also in Unsafe mode', () => {
    render(
      <PendingApprovalWidget
        approvals={[rootApproval]}
        executionMode="unsafe"
        onRespond={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    expect(screen.getByText('Root Approval Required')).toBeInTheDocument();
    expect(screen.getByText('Runs as root')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /Approve root command/i }),
    ).toBeInTheDocument();
    expect(screen.getByText(/"asRoot": true/)).toBeInTheDocument();
  });

  it('gives a root approval keyboard priority over a hard approval', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <PendingApprovalWidget
        approvals={[standardApproval, hardApproval, rootApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Escape' });

    await waitFor(() => {
      expect(onRespond).toHaveBeenCalledWith('call-root', false);
    });
  });

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
      screen.getByRole('button', { name: /Approve high-risk action/i }),
    ).toBeInTheDocument();
  });

  it('approves the priority approval on Cmd/Ctrl+Enter', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <PendingApprovalWidget
        approvals={[standardApproval, hardApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Enter', metaKey: true });

    await waitFor(() => {
      expect(onRespond).toHaveBeenCalledWith('call-hard', true);
    });
  });

  it('does not approve on bare Enter', async () => {
    const onRespond = vi.fn().mockResolvedValue(undefined);

    render(
      <PendingApprovalWidget
        approvals={[standardApproval, hardApproval]}
        executionMode="normal"
        onRespond={onRespond}
      />,
    );

    fireEvent.keyDown(window, { key: 'Enter' });
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(onRespond).not.toHaveBeenCalled();
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

  it('ignores Cmd/Ctrl+Enter while typing in an input', async () => {
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
    fireEvent.keyDown(input, { key: 'Enter', metaKey: true });

    expect(onRespond).not.toHaveBeenCalled();
  });
});
