import { render, screen, fireEvent, act } from '@testing-library/react';
import { SessionCard } from '../SessionCard';
import type { AgentSession } from '@/models/agent';
import { describe, it, expect, vi } from 'vitest';
import '@testing-library/jest-dom';

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
};

describe('SessionCard', () => {
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
});
