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

  // #2968: the backend masks secret values in the arguments it sends for MCP
  // server mutations (literal query values, fragments, env and header values);
  // the card must render that payload with the `{{secret:NAME}}` templates
  // still visible and no literal secret anywhere on screen.
  it('shows masked URL values and keeps secret templates in the arguments block', () => {
    const maskedArguments = JSON.stringify({
      name: 'svc',
      transport: {
        type: 'http',
        url: 'http://127.0.0.1:9001/mcp?token=***&key={{secret:URLKEY}}#***',
        headers: { Authorization: '***', 'X-Key': '{{secret:HDRKEY}}' },
      },
    });

    const { container } = render(
      <PendingApprovalWidget
        approvals={[
          {
            toolCallId: 'call-register',
            toolName: 'tool__registerServer',
            arguments: maskedArguments,
            approvalKind: 'hard',
            requestId: 'req-register',
            description: 'Hard approval',
            inputPreview: '[svc] http: http://127.0.0.1:9001/mcp?token=***',
          },
        ]}
        executionMode="normal"
        onRespond={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const card = container.textContent ?? '';
    expect(card).toContain('token=***');
    expect(card).toContain('key={{secret:URLKEY}}');
    expect(card).toContain('#***');
    expect(card).toContain('"X-Key": "{{secret:HDRKEY}}"');
    expect(card).not.toContain('literal-secret');
    expect(card).not.toContain('literal-header');
  });
});
