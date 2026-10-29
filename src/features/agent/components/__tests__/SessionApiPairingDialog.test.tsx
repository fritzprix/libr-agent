import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { SessionApiPairingDialog } from '../SessionApiPairingDialog';
import * as sessionApiBackend from '@/lib/backend/session-api-keys';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string, options?: Record<string, unknown>) => {
      let text = fallback ?? key;
      if (options) {
        for (const [k, v] of Object.entries(options)) {
          text = text.replace(new RegExp(`{{${k}}}`, 'g'), String(v));
        }
      }
      return text;
    },
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

const mockListen = vi.fn();
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

describe('SessionApiPairingDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(() => {});
  });

  it('does not render when no pending pairing request exists', async () => {
    vi.spyOn(sessionApiBackend, 'getPendingSessionApiPairing').mockResolvedValue(null);

    const { container } = render(<SessionApiPairingDialog />);
    await waitFor(() => {
      expect(container).toBeEmptyDOMElement();
    });
  });

  it('renders dialog when pending request exists and handles approval', async () => {
    vi.spyOn(sessionApiBackend, 'getPendingSessionApiPairing').mockResolvedValue({
      id: 'req-1',
      name: 'Cursor External',
      expiresInSeconds: 120,
    });
    const approveSpy = vi.spyOn(sessionApiBackend, 'approveSessionApiPairing').mockResolvedValue({
      id: 'req-1',
      name: 'Cursor External',
      createdAt: new Date().toISOString(),
    });

    render(<SessionApiPairingDialog />);

    await waitFor(() => {
      expect(screen.getByText(/Agent Pairing Request/i)).toBeInTheDocument();
      expect(screen.getByText(/Cursor External/i)).toBeInTheDocument();
    });

    const input = screen.getByPlaceholderText(/abc-123/i);
    fireEvent.change(input, { target: { value: 'xyz-789' } });

    const approveButton = screen.getByRole('button', { name: /Approve/i });
    fireEvent.click(approveButton);

    expect(approveSpy).toHaveBeenCalledWith('xyz-789');
  });

  it('handles rejection when reject button is clicked', async () => {
    vi.spyOn(sessionApiBackend, 'getPendingSessionApiPairing').mockResolvedValue({
      id: 'req-2',
      name: 'Another Agent',
      expiresInSeconds: 100,
    });
    const rejectSpy = vi.spyOn(sessionApiBackend, 'rejectSessionApiPairing').mockResolvedValue();

    render(<SessionApiPairingDialog />);

    await waitFor(() => {
      expect(screen.getByText(/Another Agent/i)).toBeInTheDocument();
    });

    const rejectButton = screen.getByRole('button', { name: /Reject/i });
    fireEvent.click(rejectButton);

    expect(rejectSpy).toHaveBeenCalled();
  });
});
