import type { AgentEventPayload } from './types';

/**
 * The user-visible notice to show when a workflow completes because a hook
 * stopped the turn (`continue: false`, #3004 G3.1), or `null` for every other
 * completion. The reason text is the plugin hook's own `stopReason`, exactly
 * what #3004 requires the user to see at a non-error end of turn.
 */
export function hookStoppedTurnNotice(
  payload: Extract<AgentEventPayload, { type: 'workflowCompleted' }>,
): string | null {
  if (payload.reason !== 'hookStopped') {
    return null;
  }
  const reason = payload.stopReason?.trim();
  return reason ? reason : null;
}
