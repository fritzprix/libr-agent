import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
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

  it('renders empty list state when no keys are returned', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockResolvedValue([]);

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(
        screen.getByText(/No API keys issued yet/i),
      ).toBeInTheDocument();
    });
  });

  it('renders active and revoked keys and allows revoking active key', async () => {
    vi.spyOn(sessionApiBackend, 'listSessionApiKeys').mockResolvedValue([
      {
        id: 'key-1',
        name: 'Cursor Agent',
        createdAt: new Date().toISOString(),
        revokedAt: null,
      },
      {
        id: 'key-2',
        name: 'Old Agent',
        createdAt: new Date().toISOString(),
        revokedAt: new Date().toISOString(),
      },
    ]);
    const revokeSpy = vi.spyOn(sessionApiBackend, 'revokeSessionApiKey').mockResolvedValue();

    render(<SessionApiKeysSection />);

    await waitFor(() => {
      expect(screen.getByText('Cursor Agent')).toBeInTheDocument();
      expect(screen.getByText('Old Agent')).toBeInTheDocument();
      expect(screen.getByText('Active')).toBeInTheDocument();
      expect(screen.getByText('Revoked')).toBeInTheDocument();
    });

    const revokeButton = screen.getByRole('button', { name: /Revoke/i });
    fireEvent.click(revokeButton);

    await waitFor(() => {
      expect(revokeSpy).toHaveBeenCalledWith('key-1');
    });
  });
});
