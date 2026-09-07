import type { PendingApprovalKind } from '@/models/agent-ipc';
import type { ExecutionMode } from './types';

/**
 * Mirrors backend `ExecutionMode::include_hard_approvals` +
 * `pending_approval_is_auto_approvable_in_yolo`.
 *
 * Used to reconcile frontend pending widgets after a successful mode change
 * without waiting for `toolExecutionApprovalResolved` events.
 */
export function isPendingApprovalAutoResolvedByMode(
  mode: ExecutionMode,
  approvalKind: PendingApprovalKind,
): boolean {
  if (mode === 'unsafe') {
    return true;
  }
  if (mode === 'yolo') {
    return approvalKind !== 'hard';
  }
  return false;
}

function isStringArray(value: unknown): value is string[] {
  return (
    Array.isArray(value) && value.every((item) => typeof item === 'string')
  );
}

/**
 * Prefer IDs returned by `agent_set_execution_mode`. Fall back to the local
 * snapshot when older backends or tests return void / an invalid payload.
 * An empty array is trusted: the backend drained nothing.
 */
export function resolvedApprovalIdsFromModeChange(
  response: unknown,
  fallbackIds: string[],
): string[] {
  if (response === undefined || response === null) {
    return fallbackIds;
  }
  if (!isStringArray(response)) {
    return fallbackIds;
  }
  return response;
}
