import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom';

import { AgentScratchpadPanel } from '../AgentScratchpadPanel';

// Mock contexts
vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: {
      id: 'session-123',
    },
  }),
}));

const mockUpdateServiceContexts = vi.fn();
const mockStructuredState = {
  items: [
    { id: 1, title: 'Test Note 1', content: 'Note content 1' },
    { id: 2, title: 'Test Note 2', content: 'Note content 2' },
  ],
};

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    serviceContexts: {
      scratchpad: {
        structuredState: mockStructuredState,
      },
    },
    updateServiceContexts: mockUpdateServiceContexts,
  }),
}));

vi.mock('../SessionSchedulesSection', () => ({
  SessionSchedulesSection: () => <div data-testid="mock-schedules-section">Schedules Section</div>,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue ?? key,
  }),
}));

describe('AgentScratchpadPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders scratchpad notes and schedules section', () => {
    render(<AgentScratchpadPanel isVisible={true} />);

    // Check title/header
    expect(screen.getByText('Schedules & Notes')).toBeInTheDocument();

    // Check scratchpad notes rendering
    expect(screen.getByText('Test Note 1')).toBeInTheDocument();
    expect(screen.getByText('Note content 1')).toBeInTheDocument();
    expect(screen.getByText('Test Note 2')).toBeInTheDocument();
    expect(screen.getByText('Note content 2')).toBeInTheDocument();

    // Check Schedules section rendering
    expect(screen.getByTestId('mock-schedules-section')).toBeInTheDocument();
  });

  it('triggers updateServiceContexts on mount if visible', () => {
    render(<AgentScratchpadPanel isVisible={true} />);
    expect(mockUpdateServiceContexts).toHaveBeenCalled();
  });
});
