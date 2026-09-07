import { describe, expect, it } from 'vitest';
import { isPendingApprovalAutoResolvedByMode } from '../executionModeApprovals';

describe('isPendingApprovalAutoResolvedByMode', () => {
  it('does not auto-resolve any pending approval in normal mode', () => {
    expect(isPendingApprovalAutoResolvedByMode('normal', 'standard')).toBe(
      false,
    );
    expect(isPendingApprovalAutoResolvedByMode('normal', 'hard')).toBe(false);
  });

  it('auto-resolves standard approvals in yolo mode but keeps hard approvals', () => {
    expect(isPendingApprovalAutoResolvedByMode('yolo', 'standard')).toBe(true);
    expect(isPendingApprovalAutoResolvedByMode('yolo', 'hard')).toBe(false);
  });

  it('auto-resolves standard and hard approvals in unsafe mode', () => {
    expect(isPendingApprovalAutoResolvedByMode('unsafe', 'standard')).toBe(
      true,
    );
    expect(isPendingApprovalAutoResolvedByMode('unsafe', 'hard')).toBe(true);
  });
});
