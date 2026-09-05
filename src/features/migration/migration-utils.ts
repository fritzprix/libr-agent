import type { CompatibilityStatus } from '@/lib/backend/migration';

export type CompatibilityKind = 'compatible' | 'newer' | 'incompatible';

export type CompatibilityBadgeVariant = 'default' | 'secondary' | 'destructive';

const COMPATIBILITY_BADGE_VARIANT: Record<
  CompatibilityKind,
  CompatibilityBadgeVariant
> = {
  compatible: 'default',
  newer: 'secondary',
  incompatible: 'destructive',
};

export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export function getCompatibilityKind(
  compatibility: CompatibilityStatus,
): CompatibilityKind {
  if (compatibility === 'Compatible') {
    return 'compatible';
  }
  if (typeof compatibility === 'object' && 'NewerVersion' in compatibility) {
    return 'newer';
  }
  return 'incompatible';
}

export function getCompatibilityBadgeVariant(
  compatibility: CompatibilityStatus,
): CompatibilityBadgeVariant {
  return COMPATIBILITY_BADGE_VARIANT[getCompatibilityKind(compatibility)];
}

export function getCompatibilityWarningMessage(
  compatibility: CompatibilityStatus,
): string | null {
  if (typeof compatibility !== 'object') {
    return null;
  }
  if ('NewerVersion' in compatibility) {
    return compatibility.NewerVersion.message;
  }
  if ('Incompatible' in compatibility) {
    return compatibility.Incompatible.message;
  }
  return null;
}

export function isIncompatible(compatibility: CompatibilityStatus): boolean {
  return getCompatibilityKind(compatibility) === 'incompatible';
}
