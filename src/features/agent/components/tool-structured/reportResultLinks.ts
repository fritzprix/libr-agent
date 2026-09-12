import { isSafeExternalUrl } from '../AgentMessageRenderer/utils/url';
import {
  fileNameFromPath,
  isWorkspaceRelativePath,
} from '../workspace-panel/filePreview';
import type { DeliverableItem } from './types';

export type ReportResultLinkAction =
  | { kind: 'external'; url: string }
  | { kind: 'workspace'; path: string }
  | { kind: 'host'; absolutePath: string }
  | { kind: 'blocked' };

function decodeHref(href: string): string {
  try {
    return decodeURIComponent(href.trim());
  } catch {
    return href.trim();
  }
}

function stripFileScheme(href: string): string {
  if (!href.toLowerCase().startsWith('file:')) {
    return href;
  }
  // file:///path or file://localhost/path → /path
  const withoutScheme = href.replace(/^file:\/\//i, '');
  if (withoutScheme.startsWith('localhost/')) {
    return `/${withoutScheme.slice('localhost/'.length)}`;
  }
  if (withoutScheme.startsWith('/')) {
    return withoutScheme;
  }
  return `/${withoutScheme}`;
}

function normalizeWorkspaceCandidate(href: string): string {
  let path = href.replace(/\\/g, '/');
  if (path.startsWith('./')) {
    path = path.slice(2);
  }
  // Common agent/container prefix; treat as workspace-relative when present.
  if (path.startsWith('/workspace/')) {
    path = path.slice('/workspace/'.length);
  }
  return path;
}

function isHttpLike(href: string): boolean {
  const lower = href.toLowerCase();
  return (
    lower.startsWith('http://') ||
    lower.startsWith('https://') ||
    lower.startsWith('mailto:') ||
    lower.startsWith('tel:')
  );
}

function hasDangerousScheme(href: string): boolean {
  // Block explicit URI schemes (javascript:, data:, …) while allowing
  // Windows drive paths like `C:\foo` / `C:/foo`.
  const windowsDrive = /^[a-zA-Z]:[\\/]/.test(href);
  if (windowsDrive) {
    return false;
  }
  return /^[a-z][a-z0-9+.-]*:/i.test(href);
}

function isHostAbsolutePath(href: string): boolean {
  const normalized = href.replace(/\\/g, '/');
  if (/^[a-zA-Z]:\//.test(normalized)) {
    return true;
  }
  // POSIX absolute, but not scheme-relative //host
  return normalized.startsWith('/') && !normalized.startsWith('//');
}

/**
 * Classify a markdown href from reportResult body text.
 * Callers must always preventDefault — never allow SPA navigation.
 */
export function classifyReportResultLink(
  href: string | undefined,
  deliverables: DeliverableItem[] = [],
): ReportResultLinkAction {
  if (!href) {
    return { kind: 'blocked' };
  }

  const decoded = decodeHref(href);
  if (!decoded || decoded.startsWith('#')) {
    return { kind: 'blocked' };
  }

  if (isHttpLike(decoded)) {
    return isSafeExternalUrl(decoded)
      ? { kind: 'external', url: decoded }
      : { kind: 'blocked' };
  }

  const withoutFile = stripFileScheme(decoded);
  const hadFileScheme = withoutFile !== decoded;

  if (!hadFileScheme && hasDangerousScheme(decoded)) {
    return { kind: 'blocked' };
  }

  // Prefer an attached deliverable match (path or absolute_path).
  const matched = deliverables.find((item) => {
    const abs = item.absolute_path?.trim();
    return (
      item.path === withoutFile ||
      item.path === normalizeWorkspaceCandidate(withoutFile) ||
      (abs !== undefined && abs.length > 0 && abs === withoutFile)
    );
  });
  if (matched?.absolute_path?.trim()) {
    return { kind: 'host', absolutePath: matched.absolute_path.trim() };
  }
  if (matched) {
    return { kind: 'workspace', path: matched.path };
  }

  const workspacePath = normalizeWorkspaceCandidate(withoutFile);
  if (isWorkspaceRelativePath(workspacePath)) {
    return { kind: 'workspace', path: workspacePath };
  }

  if (isHostAbsolutePath(withoutFile)) {
    return { kind: 'host', absolutePath: withoutFile };
  }

  return { kind: 'blocked' };
}

export function displayNameForWorkspacePath(path: string): string {
  return fileNameFromPath(path);
}
