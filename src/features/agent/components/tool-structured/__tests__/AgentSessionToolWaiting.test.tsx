import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { AgentSessionToolWaiting } from '../AgentSessionToolWaiting';

const cancelDelegatedWorkflow = vi.fn();

vi.mock('@/lib/backend/agent-commands', () => ({
  cancelDelegatedWorkflow: (...args: unknown[]) =>
    cancelDelegatedWorkflow(...args),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    message: vi.fn(),
    error: vi.fn(),
  },
}));

describe('AgentSessionToolWaiting', () => {
  beforeEach(() => {
    cancelDelegatedWorkflow.mockReset();
    cancelDelegatedWorkflow.mockResolvedValue({ success: true });
  });

  it('calls cancelDelegatedWorkflow when Stop is clicked', async () => {
    render(
      <AgentSessionToolWaiting
        callerSessionId="parent-1"
        childSessionRef="child-abc"
        displayName="Helper"
      />,
    );

    expect(
      screen.getByTestId('tool-structured-check-session-waiting'),
    ).toBeInTheDocument();
    expect(screen.getByText(/Helper/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /stop/i }));

    await waitFor(() => {
      expect(cancelDelegatedWorkflow).toHaveBeenCalledWith(
        'parent-1',
        'child-abc',
      );
    });
  });
});
