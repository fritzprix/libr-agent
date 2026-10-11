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

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

vi.mock('@/features/scheduled-tasks/setupKnowledgeDistillSchedule', () => ({
  setupKnowledgeDistillSchedule: vi.fn().mockResolvedValue({
    created: true,
    assistantId: 'ast-1',
    task: { id: 'task-1' },
  }),
}));

vi.mock('swr', async () => {
  const actual = await vi.importActual<typeof import('swr')>('swr');
  return {
    ...actual,
    mutate: vi.fn(),
  };
});

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
      if (key === 'spotlight.items.knowledgeDistillSchedule.title') {
        return 'Schedule durable knowledge distillation';
      }
      if (key === 'spotlight.items.knowledgeDistillSchedule.body') {
        return 'Create a nightly distill task';
      }
      if (key === 'spotlight.items.everydayChrome.title') {
        return 'Use your everyday Chrome';
      }
      if (key === 'spotlight.items.everydayChrome.body') {
        return 'Connect the Chrome extension under Settings';
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

  it('shows a tip while busy and rotates', async () => {
    workflowStatus = 'busy';
    render(
      <MemoryRouter>
        <ComposerBusyTipStrip />
      </MemoryRouter>,
    );

    const tip = screen.getByTestId('composer-busy-tip');
    expect(tip).toHaveTextContent(
      'Tip · Schedule durable knowledge distillation',
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId('composer-busy-tip'));
    });
    expect(mockNavigate).toHaveBeenCalledWith('/scheduled-tasks');

    act(() => {
      vi.advanceTimersByTime(WAIT_TIP_ROTATE_MS);
    });

    // knowledge-distill tip was dismissed by the CTA; rotation continues
    // with the remaining wait pool (index already advanced past 0).
    expect(screen.getByTestId('composer-busy-tip')).toHaveTextContent(
      'Tip · Try starter tasks',
    );

    const stored = JSON.parse(
      localStorage.getItem(SPOTLIGHT_STATE_KEY) ?? '{}',
    ) as { waitTipIndex?: number };
    expect(stored.waitTipIndex).toBeGreaterThanOrEqual(1);
  });
});
