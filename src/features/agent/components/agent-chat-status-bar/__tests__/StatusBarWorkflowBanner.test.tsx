import { fireEvent, render, screen } from '@testing-library/react';
import '@testing-library/jest-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  shouldShowWorkflowBanner,
  StatusBarWorkflowBanner,
} from '../StatusBarWorkflowBanner';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultValueOrOptions?: string | Record<string, unknown>,
    ) =>
      typeof defaultValueOrOptions === 'string'
        ? defaultValueOrOptions
        : key,
  }),
}));

vi.mock('@/components/ui/LoadingSpinner', () => ({
  default: () => <div>spinner</div>,
}));

describe('StatusBarWorkflowBanner', () => {
  const defaultProps = {
    workflowStatus: 'idle' as const,
    error: null,
    llmError: null,
    awaitingCompact: false,
    compacting: false,
    hasPendingApprovals: false,
    isRetrying: false,
    isResuming: false,
    onRetry: vi.fn(),
    onResume: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('shouldShowWorkflowBanner', () => {
    it('returns false for idle state without errors or approvals', () => {
      expect(
        shouldShowWorkflowBanner({
          error: null,
          llmError: null,
          awaitingCompact: false,
          compacting: false,
          hasPendingApprovals: false,
          workflowStatus: 'idle',
        }),
      ).toBe(false);
    });

    it('returns true when busy', () => {
      expect(
        shouldShowWorkflowBanner({
          error: null,
          llmError: null,
          awaitingCompact: false,
          compacting: false,
          hasPendingApprovals: false,
          workflowStatus: 'busy',
        }),
      ).toBe(true);
    });

    it('returns true when error is present', () => {
      expect(
        shouldShowWorkflowBanner({
          error: {
            displayMessage: 'LLM failed',
            type: 'AI_SERVICE_ERROR',
            recoverable: true,
          },
          llmError: null,
          awaitingCompact: false,
          compacting: false,
          hasPendingApprovals: false,
          workflowStatus: 'idle',
        }),
      ).toBe(true);
    });

    it('returns true when approvals are pending', () => {
      expect(
        shouldShowWorkflowBanner({
          error: null,
          llmError: null,
          awaitingCompact: false,
          compacting: false,
          hasPendingApprovals: true,
          workflowStatus: 'idle',
        }),
      ).toBe(true);
    });
  });

  it('renders nothing when idle and no banner conditions met', () => {
    render(<StatusBarWorkflowBanner {...defaultProps} />);
    expect(
      screen.queryByTestId('agent-workflow-status'),
    ).not.toBeInTheDocument();
  });

  it('renders busy state when workflowStatus is busy', () => {
    render(
      <StatusBarWorkflowBanner
        {...defaultProps}
        workflowStatus="busy"
      />,
    );
    expect(screen.getByTestId('agent-workflow-status')).toBeInTheDocument();
  });

  it('renders paused state and fires onResume when continue button is clicked', () => {
    const onResume = vi.fn();
    render(
      <StatusBarWorkflowBanner
        {...defaultProps}
        workflowStatus="paused"
        onResume={onResume}
      />,
    );

    const continueBtn = screen.getByRole('button', { name: /continue/i });
    expect(continueBtn).toBeInTheDocument();
    fireEvent.click(continueBtn);
    expect(onResume).toHaveBeenCalledTimes(1);
  });

  it('renders error state and fires onRetry when retry button is clicked', () => {
    const onRetry = vi.fn();
    render(
      <StatusBarWorkflowBanner
        {...defaultProps}
        workflowStatus="error"
        error={{
          displayMessage: 'Connection failed',
          type: 'AI_SERVICE_ERROR',
          recoverable: true,
        }}
        onRetry={onRetry}
      />,
    );

    const retryBtn = screen.getByRole('button', { name: /retry/i });
    expect(retryBtn).toBeInTheDocument();
    fireEvent.click(retryBtn);
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('renders awaiting approval message when hasPendingApprovals is true', () => {
    render(
      <StatusBarWorkflowBanner
        {...defaultProps}
        hasPendingApprovals={true}
      />,
    );

    expect(screen.getByText(/Awaiting approval/i)).toBeInTheDocument();
  });
});
