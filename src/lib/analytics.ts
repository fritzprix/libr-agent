/**
 * Telemetry for panel interactions.
 *
 * Events always go to the app logger. An optional provider can be registered
 * for external sinks (Mixpanel, etc.) without changing call sites.
 */

import type { AgentPanelId } from '@/context/AgentPanelsContext';
import { getLogger } from './logger';

const logger = getLogger('analytics');

export type PanelEventName =
  | 'panel_viewed'
  | 'panel_action'
  | 'panel_badge_clicked'
  | 'panel_shortcut_used';

export interface PanelEventPayload {
  panel: AgentPanelId;
  trigger: string;
  sessionId?: string;
}

let analyticsProvider:
  | ((event: PanelEventName, payload: PanelEventPayload) => void)
  | null = null;

/** Optional external sink. Pass `null` to clear. Logger still records events. */
export function registerAnalyticsProvider(
  provider:
    | ((event: PanelEventName, payload: PanelEventPayload) => void)
    | null,
): void {
  analyticsProvider = provider;
}

export function track(event: PanelEventName, payload: PanelEventPayload): void {
  logger.debug('Panel event', { event, payload, timestamp: Date.now() });

  if (!analyticsProvider) {
    return;
  }

  try {
    analyticsProvider(event, payload);
  } catch (error) {
    logger.error('Analytics provider error', error);
  }
}

export function trackPanelViewed(
  panel: AgentPanelId,
  trigger: string,
  sessionId?: string,
): void {
  track('panel_viewed', { panel, trigger, sessionId });
}

export function trackPanelAction(
  panel: AgentPanelId,
  trigger: string,
  sessionId?: string,
): void {
  track('panel_action', { panel, trigger, sessionId });
}

export function trackBadgeClicked(
  panel: AgentPanelId,
  sessionId?: string,
): void {
  track('panel_badge_clicked', { panel, trigger: 'badge', sessionId });
  trackPanelViewed(panel, 'badge', sessionId);
}

export function trackShortcutUsed(
  panel: AgentPanelId,
  shortcut: string,
  sessionId?: string,
): void {
  track('panel_shortcut_used', { panel, trigger: shortcut, sessionId });
  trackPanelViewed(panel, 'shortcut', sessionId);
}
