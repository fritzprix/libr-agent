/**
 * Keyboard shortcuts for opening agent panels.
 *
 *   Cmd/Ctrl + Shift + J → Processes
 *   Cmd/Ctrl + Shift + P → Planning
 *   Cmd/Ctrl + Shift + U → Workspace
 *
 * Workspace uses U (not W) to avoid browser "close window" (Ctrl/Cmd+Shift+W).
 */

import { useCallback, useEffect } from 'react';
import {
  useAgentPanels,
  type AgentPanelId,
} from '@/context/AgentPanelsContext';
import { trackShortcutUsed } from '@/lib/analytics';
import { getLogger } from '@/lib/logger';

const logger = getLogger('usePanelShortcuts');

/** Exported for unit tests. */
export const PANEL_SHORTCUT_MAP: Record<string, AgentPanelId> = {
  j: 'processes',
  p: 'planning',
  u: 'workspace',
};

/** True when the event target is a text field or other editable surface. */
export function isEditableKeyboardTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  if (target.isContentEditable) {
    return true;
  }

  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
    return true;
  }

  return target.closest('[contenteditable="true"]') !== null;
}

/**
 * Resolve a panel id from a keyboard event, or null if it is not a panel shortcut.
 */
export function resolvePanelShortcut(
  event: KeyboardEvent,
): AgentPanelId | null {
  const modifier = event.metaKey || event.ctrlKey;
  if (!modifier || !event.shiftKey || event.altKey) {
    return null;
  }

  if (isEditableKeyboardTarget(event.target)) {
    return null;
  }

  return PANEL_SHORTCUT_MAP[event.key.toLowerCase()] ?? null;
}

export function usePanelShortcuts(): void {
  const { openPanel } = useAgentPanels();

  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      const panelId = resolvePanelShortcut(event);
      if (!panelId) {
        return;
      }

      event.preventDefault();
      const shortcut = `Cmd+Shift+${event.key.toUpperCase()}`;
      logger.info('Panel shortcut activated', { panel: panelId, shortcut });

      openPanel(panelId);
      trackShortcutUsed(panelId, shortcut);
    },
    [openPanel],
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown]);
}
