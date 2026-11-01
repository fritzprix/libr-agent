import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { SessionApiKeysSection } from '../SessionApiKeysSection';
import * as sessionApiBackend from '@/lib/backend/session-api-keys';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

describe('SessionApiKeysSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders empty list state when no keys are returned', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockResolvedValue([]);

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(
        screen.getByText(/No API keys issued yet/i),
      ).toBeInTheDocument();
    });
  });

  it('requires a two-step inline confirmation before revoking an active key', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockResolvedValue([
      {
        id: 'key-1',
        name: 'Cursor Agent',
        createdAt: new Date().toISOString(),
      },
      {
        id: 'key-2',
        name: 'Bench Agent',
        createdAt: new Date().toISOString(),
      },
    ]);
    const deleteSpy = vi
      .spyOn(sessionApiBackend, 'revokeSessionApiKey')
      .mockResolvedValue();

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(screen.getByText('Cursor Agent')).toBeInTheDocument();
      expect(screen.getByText('Bench Agent')).toBeInTheDocument();
      expect(screen.getAllByText('Active')).toHaveLength(2);
    });

    const deleteButtons = screen.getAllByRole('button', { name: /Delete/i });
    fireEvent.click(deleteButtons[0]);

    expect(deleteSpy).not.toHaveBeenCalled();
    const confirmButton = screen.getByRole('button', {
      name: /Confirm revoke/i,
    });
    expect(confirmButton).toBeInTheDocument();

    fireEvent.click(confirmButton);

    await waitFor(() => {
      expect(deleteSpy).toHaveBeenCalledWith('key-1');
    });
  });

  it('resets confirmation state on cancel, blur, or timeout without revoking', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockResolvedValue([
      {
        id: 'key-1',
        name: 'Cursor Agent',
        createdAt: new Date().toISOString(),
      },
    ]);
    const deleteSpy = vi
      .spyOn(sessionApiBackend, 'revokeSessionApiKey')
      .mockResolvedValue();

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(screen.getByText('Cursor Agent')).toBeInTheDocument();
    });

    // 1. Cancel resets to Delete
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));
    fireEvent.click(screen.getByRole('button', { name: /Cancel/i }));
    expect(
      screen.queryByRole('button', { name: /Confirm revoke/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Delete/i })).toBeInTheDocument();
    expect(deleteSpy).not.toHaveBeenCalled();

    // 2. Blur resets to Delete
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));
    fireEvent.blur(screen.getByRole('button', { name: /Confirm revoke/i }));
    expect(
      screen.queryByRole('button', { name: /Confirm revoke/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Delete/i })).toBeInTheDocument();
    expect(deleteSpy).not.toHaveBeenCalled();

    // 3. Timeout resets to Delete
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole('button', { name: /Delete/i }));
    expect(
      screen.getByRole('button', { name: /Confirm revoke/i }),
    ).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(
      screen.queryByRole('button', { name: /Confirm revoke/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Delete/i })).toBeInTheDocument();
    expect(deleteSpy).not.toHaveBeenCalled();
  });

  it('renders error state when loading keys fails', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockRejectedValue(
      new Error('key file must not be accessible by group or others'),
    );

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(
        screen.getByText(/Failed to load Session API keys/i),
      ).toBeInTheDocument();
      expect(
        screen.getByText(/key file must not be accessible by group or others/i),
      ).toBeInTheDocument();
      expect(
        screen.getByText(/Key verification is locked \(fail-closed\)/i),
      ).toBeInTheDocument();
    });

    expect(screen.queryByText(/No API keys issued yet/i)).not.toBeInTheDocument();
  });
});
