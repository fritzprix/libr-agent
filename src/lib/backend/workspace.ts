import { safeInvoke } from './core';
import type { WorkspaceFileItem } from './types';

/**
 * Lists the files and directories in the specified workspace path.
 * @param path The optional path within the workspace to list. Defaults to the root.
 * @returns A promise that resolves to an array of `WorkspaceFileItem` objects.
 */
export async function listWorkspaceFiles(
  path?: string,
): Promise<WorkspaceFileItem[]> {
  return safeInvoke<WorkspaceFileItem[]>(
    'list_workspace_files',
    path ? { path } : {},
  );
}

/**
 * Writes content to a file within the application's workspace directory.
 * @param filePath The relative path within the workspace to write to.
 * @param content An array of numbers representing the byte content to write.
 * @returns A promise that resolves when the write operation is complete.
 */
export async function workspaceWriteFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('workspace_write_file', {
    filePath,
    content,
  });
}
