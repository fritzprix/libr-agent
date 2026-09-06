import { describe, expect, it } from 'vitest';
import type { CompatibilityStatus } from '@/lib/backend/migration';
import {
  formatBytes,
  getCompatibilityBadgeVariant,
  getCompatibilityKind,
  getCompatibilityWarningMessage,
  isIncompatible,
} from '../migration-utils';

describe('migration-utils', () => {
  describe('formatBytes', () => {
    it('formats zero bytes', () => {
      expect(formatBytes(0)).toBe('0 Bytes');
    });

    it('formats kilobytes and megabytes', () => {
      expect(formatBytes(1024)).toBe('1 KB');
      expect(formatBytes(1048576)).toBe('1 MB');
    });
  });

  describe('compatibility helpers', () => {
    const compatible: CompatibilityStatus = 'Compatible';
    const newer: CompatibilityStatus = {
      NewerVersion: { message: 'newer message' },
    };
    const incompatible: CompatibilityStatus = {
      Incompatible: { message: 'incompatible message' },
    };

    it('maps compatibility kinds and badge variants', () => {
      expect(getCompatibilityKind(compatible)).toBe('compatible');
      expect(getCompatibilityKind(newer)).toBe('newer');
      expect(getCompatibilityKind(incompatible)).toBe('incompatible');

      expect(getCompatibilityBadgeVariant(compatible)).toBe('default');
      expect(getCompatibilityBadgeVariant(newer)).toBe('secondary');
      expect(getCompatibilityBadgeVariant(incompatible)).toBe('destructive');
    });

    it('extracts warning messages and incompatible flag', () => {
      expect(getCompatibilityWarningMessage(compatible)).toBeNull();
      expect(getCompatibilityWarningMessage(newer)).toBe('newer message');
      expect(getCompatibilityWarningMessage(incompatible)).toBe(
        'incompatible message',
      );
      expect(isIncompatible(compatible)).toBe(false);
      expect(isIncompatible(incompatible)).toBe(true);
    });
  });
});
