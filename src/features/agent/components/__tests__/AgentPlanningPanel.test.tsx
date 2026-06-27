import { render, act } from '@testing-library/react';
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

  it('refreshes planning contexts when it becomes visible', async () => {
    const { rerender } = render(<AgentPlanningPanel isVisible={false} />);

    expect(mockUpdateServiceContexts).not.toHaveBeenCalled();

    await act(async () => {
      rerender(<AgentPlanningPanel isVisible />);
    });

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);
  });

  it('does not refresh again on rerender while still visible', async () => {
    let rerender: any;
    await act(async () => {
      const res = render(<AgentPlanningPanel isVisible />);
      rerender = res.rerender;
    });

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);

    await act(async () => {
      rerender(<AgentPlanningPanel isVisible />);
    });

    expect(mockUpdateServiceContexts).toHaveBeenCalledTimes(1);
  });
});
