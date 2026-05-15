import { render } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { AgentPlanningPanel } from '../AgentPlanningPanel';

const mockUpdateServiceContexts = vi.fn();

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: {
      id: 'session-1',
    },
  }),
}));

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    serviceContexts: {},
    updateServiceContexts: mockUpdateServiceContexts,
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('AgentPlanningPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('refreshes planning contexts when it becomes visible', () => {
    const { rerender } = render(<AgentPlanningPanel isVisible={false} />);

    expect(mockUpdateServiceContexts).not.toHaveBeenCalled();

    rerender(<AgentPlanningPanel isVisible />);

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);
  });

  it('does not refresh again on rerender while still visible', () => {
    const { rerender } = render(<AgentPlanningPanel isVisible />);

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);

    rerender(<AgentPlanningPanel isVisible />);

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);
  });
});
