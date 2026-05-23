function normalizeExtendedWindowsPath(path: string): string {
  const normalized = path.replace(/\\/g, '/');

  const extendedUncMatch = normalized.match(/^\/\/\?\/UNC\/(.+)$/i);
  if (extendedUncMatch) {
    return `//${extendedUncMatch[1]}`;
  }

  const extendedPathMatch = normalized.match(/^\/\/\?\/(.+)$/);
  if (extendedPathMatch) {
    return extendedPathMatch[1];
  }

  return normalized;
}

function buildFileUrl(pathname: string, host?: string): string {
  const url = new URL(host ? `file://${host}` : 'file:///');
  url.pathname = pathname;
  return url.toString();
}

export function pathToFileUrl(path: string): string {
  const normalized = normalizeExtendedWindowsPath(path);

  if (/^\/\/[^/]+\/.+/.test(normalized)) {
    const withoutPrefix = normalized.slice(2);
    const firstSlashIndex = withoutPrefix.indexOf('/');

    if (firstSlashIndex > 0) {
      const host = withoutPrefix.slice(0, firstSlashIndex);
      const pathname = withoutPrefix.slice(firstSlashIndex);
      return buildFileUrl(pathname, host);
    }
  }

  if (/^[a-zA-Z]:\//.test(normalized)) {
    return buildFileUrl(`/${normalized}`);
  }

  if (normalized.startsWith('/')) {
    return buildFileUrl(normalized);
  }

  return buildFileUrl(`/${normalized}`);
}

export function workspacePathToFileUrl(
  workspaceDir: string,
  workspacePath: string,
): string {
  const normalizedDir = workspaceDir.replace(/\\/g, '/').replace(/\/+$/, '');
  const normalizedPath = workspacePath.replace(/\\/g, '/').replace(/^\/+/, '');

  return pathToFileUrl(`${normalizedDir}/${normalizedPath}`);
}
