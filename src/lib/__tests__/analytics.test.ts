import { afterEach, describe, expect, it, vi } from 'vitest';

const mockDebug = vi.fn();
const mockError = vi.fn();

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: mockDebug,
    error: mockError,
    info: vi.fn(),
    warn: vi.fn(),
  }),
}));

const {
  registerAnalyticsProvider,
  track,
  trackBadgeClicked,
  trackPanelAction,
  trackShortcutUsed,
} = await import('@/lib/analytics');

describe('analytics', () => {
  afterEach(() => {
    vi.clearAllMocks();
    registerAnalyticsProvider(null);
  });

  it('logs every event and forwards to a registered provider', () => {
    const provider = vi.fn();
    registerAnalyticsProvider(provider);

    track('panel_action', {
      panel: 'workspace',
      trigger: 'close',
      sessionId: 's1',
    });

    expect(mockDebug).toHaveBeenCalledWith(
      'Panel event',
      expect.objectContaining({
        event: 'panel_action',
        payload: {
          panel: 'workspace',
          trigger: 'close',
          sessionId: 's1',
        },
      }),
    );
    expect(provider).toHaveBeenCalledWith('panel_action', {
      panel: 'workspace',
      trigger: 'close',
      sessionId: 's1',
    });
  });

  it('records provider errors without throwing', () => {
    registerAnalyticsProvider(() => {
      throw new Error('provider down');
    });

    expect(() => trackPanelAction('planning', 'close', 's1')).not.toThrow();
    expect(mockError).toHaveBeenCalled();
  });

  it('emits badge_clicked and panel_viewed for badge opens', () => {
    const provider = vi.fn();
    registerAnalyticsProvider(provider);

    trackBadgeClicked('processes', 's1');

    expect(provider).toHaveBeenCalledWith('panel_badge_clicked', {
      panel: 'processes',
      trigger: 'badge',
      sessionId: 's1',
    });
    expect(provider).toHaveBeenCalledWith('panel_viewed', {
      panel: 'processes',
      trigger: 'badge',
      sessionId: 's1',
    });
  });

  it('emits shortcut_used and panel_viewed for shortcut opens', () => {
    const provider = vi.fn();
    registerAnalyticsProvider(provider);

    trackShortcutUsed('workspace', 'Cmd+Shift+U', 's1');

    expect(provider).toHaveBeenCalledWith('panel_shortcut_used', {
      panel: 'workspace',
      trigger: 'Cmd+Shift+U',
      sessionId: 's1',
    });
    expect(provider).toHaveBeenCalledWith('panel_viewed', {
      panel: 'workspace',
      trigger: 'shortcut',
      sessionId: 's1',
    });
  });
});
