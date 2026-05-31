import { render, screen, fireEvent, act } from '@testing-library/react';
import { SessionCard } from '../SessionCard';
import type { AgentSession } from '@/models/agent';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultString: string, options?: Record<string, unknown>) => {
      if (key === 'sessionHistory.actions.deleteAria') {
        return `Delete session ${options?.name || ''}`;
      }
      if (key === 'sessionHistory.actions.deleteTooltip') {
        return 'Delete session';
      }
      return defaultString || key;
    },
  }),
}));

// Mock ResizeObserver for Radix UI
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

const mockSession: AgentSession = {
  id: 'session-123',
  name: 'Test Session',
  status: 'idle',
  model: 'gpt-4',
  provider: 'openai',
  createdAt: new Date(),
  updatedAt: new Date(),
  assistant: undefined,
  yoloMode: false,
};

describe('SessionCard', () => {
  it('applies stronger card-level styling for error sessions', () => {
    const onResume = vi.fn();
    const onDelete = vi.fn();

    render(
      <SessionCard
        session={{ ...mockSession, status: 'error' }}
        onResume={onResume}
        onDelete={onDelete}
      />,
    );

    const card = screen.getByText('Test Session').closest('article');
    expect(card).not.toBeNull();
    expect(card).toHaveClass('bg-destructive/5');
    expect(card).toHaveClass('border-destructive/30');
  });

  it('displays tooltip on delete button hover', async () => {
    const onResume = vi.fn();
    const onDelete = vi.fn();

    render(
      <SessionCard
        session={mockSession}
        onResume={onResume}
        onDelete={onDelete}
      />
    );

    const deleteButton = screen.getByLabelText(/Delete session Test Session/i);
    expect(deleteButton).toBeInTheDocument();

    // Radix Tooltip might need focus as well as hover
    act(() => {
      deleteButton.focus();
      fireEvent.mouseOver(deleteButton);
    });

    // We use getAllByText because Radix UI renders the text twice (once visible, once in a hidden span for a11y)
    // We just want to ensure at least one of them appears.
    const tooltipElements = await screen.findAllByText(/Delete session/);
    expect(tooltipElements.length).toBeGreaterThan(0);

    // Optional: check if one is visible
    // const visibleTooltip = tooltipElements.find(el => el.checkVisibility?.());
    // expect(visibleTooltip).toBeInTheDocument();
  });

  it('shows visible bookmark state affordances for bookmarked sessions', () => {
    const onResume = vi.fn();
    const onDelete = vi.fn();
    const onToggleBookmark = vi.fn();

    render(
      <SessionCard
        session={{ ...mockSession, isBookmarked: true }}
        onResume={onResume}
        onDelete={onDelete}
        onToggleBookmark={onToggleBookmark}
      />,
    );

    expect(screen.getAllByText('Bookmarked').length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: /Remove bookmark/i }));
    expect(onToggleBookmark).toHaveBeenCalledWith('session-123');
  });
});
