import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { ComposerBusyTipStrip } from '../ComposerBusyTipStrip';
import {
  SPOTLIGHT_STATE_KEY,
  WAIT_TIP_ROTATE_MS,
} from '@/features/spotlight';

const mockNavigate = vi.fn();
let workflowStatus: 'idle' | 'busy' = 'idle';
const settingsMock = { showFeatureTips: true };

vi.mock('react-router-dom', async () => {
  const actual =
    await vi.importActual<typeof import('react-router-dom')>(
      'react-router-dom',
    );
  return {
    ...actual,
    useNavigate: () => mockNavigate,
  };
});

vi.mock('@/context/AgentChatContext', () => ({
  useAgentChat: () => ({
    get workflowStatus() {
      return workflowStatus;
    },
  }),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      display: {
        get showFeatureTips() {
          return settingsMock.showFeatureTips;
        },
      },
    },
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string; title?: string }) => {
      if (key === 'spotlight.waitLine' && options?.title) {
        return `Tip · ${options.title}`;
      }
      if (key === 'spotlight.items.browserProfile.title') {
        return 'Import browser login sessions';
      }
      if (key === 'spotlight.items.starterTasks.title') {
        return 'Try starter tasks';
      }
      if (key === 'spotlight.items.browserProfile.body') {
        return 'Bring profiles into LibrAgent';
      }
      return options?.defaultValue ?? key;
    },
  }),
}));

describe('ComposerBusyTipStrip', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    localStorage.clear();
    mockNavigate.mockReset();
    workflowStatus = 'idle';
    settingsMock.showFeatureTips = true;
  });

  afterEach(() => {
    vi.useRealTimers();
    localStorage.clear();
  });

  it('hides when workflow is idle', () => {
    workflowStatus = 'idle';
    render(
      <MemoryRouter>
        <ComposerBusyTipStrip />
      </MemoryRouter>,
    );
    expect(screen.queryByTestId('composer-busy-tip-strip')).toBeNull();
  });

  it('shows a tip while busy and rotates', () => {
    workflowStatus = 'busy';
    render(
      <MemoryRouter>
        <ComposerBusyTipStrip />
      </MemoryRouter>,
    );

    const tip = screen.getByTestId('composer-busy-tip');
    expect(tip).toHaveTextContent('Tip · Import browser login sessions');

    act(() => {
      vi.advanceTimersByTime(WAIT_TIP_ROTATE_MS);
    });

    expect(screen.getByTestId('composer-busy-tip')).toHaveTextContent(
      'Tip · Try starter tasks',
    );

    fireEvent.click(screen.getByTestId('composer-busy-tip'));
    expect(mockNavigate).toHaveBeenCalledWith('/scheduled-tasks');

    const stored = JSON.parse(
      localStorage.getItem(SPOTLIGHT_STATE_KEY) ?? '{}',
    ) as { waitTipIndex?: number };
    expect(stored.waitTipIndex).toBeGreaterThanOrEqual(2);
  });
});
