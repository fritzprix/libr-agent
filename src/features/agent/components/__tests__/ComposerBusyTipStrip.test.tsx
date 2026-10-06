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
  useAgentChatState: () => ({
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
      if (key === 'spotlight.items.starterTasks.title') {
        return 'Try starter tasks';
      }
      if (key === 'spotlight.items.starterTasks.body') {
        return 'One-click automation starters are ready';
      }
      return options?.defaultValue ?? key;
    },
  }),
}));

function createMemoryStorage() {
  const store = new Map<string, string>();
  return {
    getItem: vi.fn((key: string) => store.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store.set(key, String(value));
    }),
    removeItem: vi.fn((key: string) => {
      store.delete(key);
    }),
    clear: vi.fn(() => {
      store.clear();
    }),
  };
}

describe('ComposerBusyTipStrip', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('localStorage', createMemoryStorage());
    mockNavigate.mockReset();
    workflowStatus = 'idle';
    settingsMock.showFeatureTips = true;
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
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
    expect(tip).toHaveTextContent('Tip · Try starter tasks');

    fireEvent.click(screen.getByTestId('composer-busy-tip'));
    expect(mockNavigate).toHaveBeenCalledWith('/scheduled-tasks');

    act(() => {
      vi.advanceTimersByTime(WAIT_TIP_ROTATE_MS);
    });

    expect(screen.getByTestId('composer-busy-tip')).toHaveTextContent(
      'Tip · spotlight.items.thinkingEffort.title',
    );

    const stored = JSON.parse(
      localStorage.getItem(SPOTLIGHT_STATE_KEY) ?? '{}',
    ) as { waitTipIndex?: number };
    expect(stored.waitTipIndex).toBeGreaterThanOrEqual(1);
  });
});
