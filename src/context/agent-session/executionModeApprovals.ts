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
