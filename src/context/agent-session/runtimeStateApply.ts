import type { SessionRuntimeState } from '@/models/agent-ipc';

/** Prefer the newer snapshot by monotonic sequence (ties keep the open/event payload). */
export function shouldApplyRuntimeState(
  currentState: SessionRuntimeState,
  nextState: SessionRuntimeState,
): boolean {
  return nextState.sequence >= currentState.sequence;
}

/** Pick which runtime state wins when reconciling open() with live event state. */
export function pickRuntimeState(
  currentState: SessionRuntimeState,
  nextState: SessionRuntimeState,
): SessionRuntimeState {
  return shouldApplyRuntimeState(currentState, nextState)
    ? nextState
    : currentState;
}
