import { isPreviewable } from './fileIconUtils';

/** Files larger than this skip the in-app preview sheet and open in the OS app. */
export const PREVIEW_MAX_BYTES = 2 * 1024 * 1024;

/**
 * Returns the last path segment (file name) from a POSIX or Windows path.
 */
export function fileNameFromPath(filePath: string): string {
  const normalized = filePath.replace(/\\/g, '/');
  const parts = normalized.split('/');
  return parts[parts.length - 1] || filePath;
}

/**
 * True when `path` is a workspace-relative location (not an OS absolute,
 * home, UNC, or `..` traversal path). Supports authorized alias prefixes
 * (@teamwork, .libragent/teamwork, @skills).
 *
 * In-app preview reads via `read_workspace_file_content`, which is scoped to the
 * session workspace root and authorized alias roots. External writes (e.g. `/tmp/out.md`)
 * must open in the host default application instead.
 */
export function isWorkspaceRelativePath(path: string): boolean {
  const trimmed = path.trim();
  if (!trimmed) return false;

  let normalized = trimmed.replace(/\\/g, '/');
  if (
    normalized === '/@teamwork' ||
    normalized.startsWith('/@teamwork/') ||
    normalized === '/.libragent/teamwork' ||
    normalized.startsWith('/.libragent/teamwork/') ||
    normalized === '/@skills' ||
    normalized.startsWith('/@skills/')
  ) {
    normalized = normalized.slice(1);
  }
  if (normalized.startsWith('/') || normalized.startsWith('~')) {
    return false;
  }
  if (/^[a-zA-Z]:/.test(normalized)) {
    return false;
  }

  const segments = normalized.split('/');
  return !segments.some((segment) => segment === '..');
}

export interface PreviewEligibilityInput {
  path: string;
  size?: number | null;
}

/**
 * Whether a file can be opened in `WorkspaceFilePreviewSheet`.
 */
export function canOpenInAppPreview({
  path,
  size,
}: PreviewEligibilityInput): boolean {
  if (!isWorkspaceRelativePath(path)) {
    return false;
  }
  if (
    size !== undefined &&
    size !== null &&
    Number.isFinite(size) &&
    size > PREVIEW_MAX_BYTES
  ) {
    return false;
  }
  return isPreviewable(fileNameFromPath(path));
}
