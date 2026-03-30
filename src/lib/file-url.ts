export function pathToFileUrl(path: string): string {
  const normalized = path.replace(/\\/g, '/');

  if (/^[a-zA-Z]:\//.test(normalized)) {
    return encodeURI(`file:///${normalized}`);
  }

  if (normalized.startsWith('/')) {
    return encodeURI(`file://${normalized}`);
  }

  return encodeURI(`file://${normalized}`);
}

export function workspacePathToFileUrl(
  workspaceDir: string,
  workspacePath: string,
): string {
  const normalizedDir = workspaceDir.replace(/\\/g, '/').replace(/\/+$/, '');
  const normalizedPath = workspacePath.replace(/\\/g, '/').replace(/^\/+/, '');

  return pathToFileUrl(`${normalizedDir}/${normalizedPath}`);
}
