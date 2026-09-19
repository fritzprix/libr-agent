import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionSchedulesSection } from '../SessionSchedulesSection';
import type { SessionScheduledTask } from '@/lib/backend/scheduled-tasks';

const cancelTask = vi.fn().mockResolvedValue(undefined);
const toggleTask = vi.fn().mockResolvedValue(undefined);

const tasks: SessionScheduledTask[] = [
  {
    id: 'task-active',
    name: 'Active callback',
    message: 'Ping soon',
    sessionId: 'session-1',
    isOneShot: true,
    enabled: true,
    nextRunAt: Date.now() + 45_000,
  },
  {
    id: 'task-paused',
    name: 'Paused callback',
    message: 'Hold this',
    sessionId: 'session-1',
    isOneShot: false,
    enabled: false,
    nextRunAt: Date.now() + 120_000,
  },
];

vi.mock('../../hooks/useSessionSchedules', () => ({
  useSessionSchedules: () => ({
    tasks,
    loading: false,
    cancellingIds: new Set<string>(),
    togglingIds: new Set<string>(),
    cancelTask,
    toggleTask,
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

describe('SessionSchedulesSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders enabled and paused session callbacks with toggle controls', () => {
    render(<SessionSchedulesSection sessionId="session-1" />);

    expect(screen.getByText('Active callback')).toBeInTheDocument();
    expect(screen.getByText('Paused callback')).toBeInTheDocument();
    expect(screen.getByText('Paused')).toBeInTheDocument();

    const switches = screen.getAllByRole('switch');
    expect(switches).toHaveLength(2);
    expect(switches[0]).toHaveAttribute('aria-checked', 'true');
    expect(switches[1]).toHaveAttribute('aria-checked', 'false');
  });

  it('pauses an enabled callback from the switch', () => {
    render(<SessionSchedulesSection sessionId="session-1" />);

    fireEvent.click(screen.getByRole('switch', { name: 'Pause Active callback' }));
    expect(toggleTask).toHaveBeenCalledWith(tasks[0]);
  });

  it('cancels a callback from the remove button', () => {
    render(<SessionSchedulesSection sessionId="session-1" />);

    fireEvent.click(
      screen.getByRole('button', { name: 'Cancel Paused callback' }),
    );
    expect(cancelTask).toHaveBeenCalledWith('task-paused');
  });
});
