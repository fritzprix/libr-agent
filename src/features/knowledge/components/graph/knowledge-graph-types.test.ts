import { describe, expect, it } from 'vitest';
import { segmentIntersectsAabb } from './knowledge-graph-types';

describe('segmentIntersectsAabb', () => {
  it('returns true when either endpoint is inside the AABB', () => {
    expect(segmentIntersectsAabb(5, 5, 50, 50, 0, 0, 10, 10)).toBe(true);
    expect(segmentIntersectsAabb(-10, -10, 5, 5, 0, 0, 10, 10)).toBe(true);
  });

  it('returns true when both endpoints are outside but the segment crosses the AABB', () => {
    // Horizontal line across the box
    expect(segmentIntersectsAabb(-10, 5, 20, 5, 0, 0, 10, 10)).toBe(true);
    // Vertical line across the box
    expect(segmentIntersectsAabb(5, -10, 5, 20, 0, 0, 10, 10)).toBe(true);
    // Diagonal crossing
    expect(segmentIntersectsAabb(-5, -5, 15, 15, 0, 0, 10, 10)).toBe(true);
  });

  it('returns false when the segment misses the AABB entirely', () => {
    expect(segmentIntersectsAabb(-10, -10, -5, -5, 0, 0, 10, 10)).toBe(false);
    expect(segmentIntersectsAabb(20, 20, 30, 30, 0, 0, 10, 10)).toBe(false);
    expect(segmentIntersectsAabb(-10, 5, -5, 5, 0, 0, 10, 10)).toBe(false);
  });
});
