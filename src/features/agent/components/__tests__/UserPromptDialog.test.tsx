import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import UserPromptDialog from '../UserPromptDialog';
import type { PendingUserPromptSummary } from '@/context/agent-session/types';

const mocks = vi.hoisted(() => ({
  submitUserPrompt: vi.fn(),
  cancelUserPrompt: vi.fn(),
  toastError: vi.fn(),
  loggerError: vi.fn(),
}));

vi.mock('@/lib/backend/agent-commands', () => ({
  submitUserPrompt: (...args: unknown[]) => mocks.submitUserPrompt(...args),
  cancelUserPrompt: (...args: unknown[]) => mocks.cancelUserPrompt(...args),
}));

vi.mock('sonner', () => ({
  toast: { error: (...args: unknown[]) => mocks.toastError(...args) },
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    debug: vi.fn(),
    warn: vi.fn(),
    error: (...args: unknown[]) => mocks.loggerError(...args),
  }),
}));

const secretPrompt: PendingUserPromptSummary = {
  requestId: 'req-secret',
  kind: 'secret',
  payload: {
    kind: 'secret',
    variableName: 'GITHUB_TOKEN',
    label: 'GitHub token for this repo',
    reuseNote: 'This value is reused for this session.',
  },
  createdAt: '2026-11-04T00:00:00.000Z',
};

const questionPrompt: PendingUserPromptSummary = {
  requestId: 'req-question',
  kind: 'question',
  payload: {
    kind: 'question',
    title: 'Which branch?',
    body: 'Pick the branch to deploy.',
  },
  createdAt: '2026-11-04T00:00:00.000Z',
};

describe('UserPromptDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.submitUserPrompt.mockResolvedValue({ success: true });
    mocks.cancelUserPrompt.mockResolvedValue({ success: true });
  });

  it('masks a secret value and submits it for the request id', async () => {
    render(
      <UserPromptDialog sessionId="session-1" promptState={secretPrompt} />,
    );

    expect(screen.getByText('GitHub token for this repo')).toBeInTheDocument();
    expect(
      screen.getByText('This value is reused for this session.'),
    ).toBeInTheDocument();

    const input = screen.getByLabelText('GITHUB_TOKEN');
    expect(input).toHaveAttribute('type', 'password');

    fireEvent.change(input, { target: { value: 'ghp_supersecret' } });
    fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

    await waitFor(() => {
      expect(mocks.submitUserPrompt).toHaveBeenCalledWith(
        'session-1',
        'req-secret',
        'ghp_supersecret',
      );
    });
  });

  it('renders a free-text area for a question prompt', async () => {
    render(
      <UserPromptDialog sessionId="session-1" promptState={questionPrompt} />,
    );

    expect(screen.getByText('Which branch?')).toBeInTheDocument();
    expect(screen.getByText('Pick the branch to deploy.')).toBeInTheDocument();

    const input = screen.getByLabelText('Your answer');
    expect(input.tagName).toBe('TEXTAREA');

    fireEvent.change(input, { target: { value: 'release/0.9' } });
    fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

    await waitFor(() => {
      expect(mocks.submitUserPrompt).toHaveBeenCalledWith(
        'session-1',
        'req-question',
        'release/0.9',
      );
    });
  });

  it('cancels through the backend instead of dismissing silently', async () => {
    render(
      <UserPromptDialog sessionId="session-1" promptState={secretPrompt} />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));

    await waitFor(() => {
      expect(mocks.cancelUserPrompt).toHaveBeenCalledWith(
        'session-1',
        'req-secret',
      );
    });
  });

  it('stays closed when no prompt waits', () => {
    render(<UserPromptDialog sessionId="session-1" promptState={null} />);

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('keeps a failed submit visible and never logs the value', async () => {
    mocks.submitUserPrompt.mockRejectedValue(new Error('backend down'));

    render(
      <UserPromptDialog sessionId="session-1" promptState={secretPrompt} />,
    );

    fireEvent.change(screen.getByLabelText('GITHUB_TOKEN'), {
      target: { value: 'ghp_supersecret' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Submit' }));

    await waitFor(() => {
      expect(mocks.toastError).toHaveBeenCalledWith(
        'Failed to send your answer.',
      );
    });

    const logged = JSON.stringify(mocks.loggerError.mock.calls);
    expect(logged).not.toContain('ghp_supersecret');
  });
});
