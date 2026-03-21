import { safeInvoke } from './core';
import type { WorkspaceFileItem } from './types';

/**
 * Lists the files and directories in the specified workspace path.
 * @param path The optional path within the workspace to list. Defaults to the root.
 * @param sessionId The optional session ID to specify which session's workspace to list.
 * @returns A promise that resolves to an array of `WorkspaceFileItem` objects.
 */
export async function listWorkspaceFiles(
  path?: string,
  sessionId?: string,
): Promise<WorkspaceFileItem[]> {
  return safeInvoke<WorkspaceFileItem[]>('list_workspace_files', {
    path: path || null,
    sessionId: sessionId || null,
  });
}

/**
 * Writes content to a file within the application's workspace directory.
 * @param filePath The relative path within the workspace to write to.
 * @param content An array of numbers representing the byte content to write.
 * @param sessionId The optional session ID to specify which session's workspace to use.
 * @returns A promise that resolves when the write operation is complete.
 */
export async function workspaceWriteFile(
  filePath: string,
  content: number[],
  sessionId?: string,
): Promise<void> {
  // Pass sessionId to Rust backend for session-aware file writing
  return safeInvoke<void>('workspace_write_file', {
    filePath,
    content,
    sessionId: sessionId || null,
  });
}

/**
 * Opens a workspace file with the system's default application.
 * @param filePath The relative path within the workspace to open.
 * @param sessionId The optional session ID to specify which session's workspace to use.
 * @returns A promise that resolves when the file is opened.
 */
export async function openWorkspaceFileWithDefaultApp(
  filePath: string,
  sessionId?: string,
): Promise<void> {
  return safeInvoke<void>('open_workspace_file_with_default_app', {
    filePath,
    sessionId,
  });
}

/**
 * Opens the session's workspace directory in the system file explorer.
 * @param sessionId The session ID to identify which workspace to open.
 * @returns A promise that resolves when the explorer is launched.
 */
export async function openWorkspaceInExplorer(
  sessionId: string,
): Promise<void> {
  return safeInvoke<void>('open_workspace_in_explorer', { sessionId });
}

/**
 * Opens the session's workspace directory in a system terminal.
 * @param sessionId The session ID to identify which workspace to open.
 * @returns A promise that resolves when the terminal is launched.
 */
export async function openWorkspaceInTerminal(
  sessionId: string,
): Promise<void> {
  return safeInvoke<void>('open_workspace_in_terminal', { sessionId });
}

/**
 * Gets the current workspace override path for a session.
 * @param sessionId The session ID to query.
 * @returns A promise that resolves to the override path string, or null if no override is set.
 */
export async function getWorkspaceOverride(
  sessionId: string,
): Promise<string | null> {
  return safeInvoke<string | null>('get_workspace_override', { sessionId });
}

/**
 * Sets a workspace override path for a session.
 * @param sessionId The session ID to configure.
 * @param overridePath The absolute path to use as the workspace directory.
 * @returns A promise that resolves when the override is set.
 */
export async function setWorkspaceOverride(
  sessionId: string,
  overridePath: string,
): Promise<void> {
  return safeInvoke<void>('set_workspace_override', {
    sessionId,
    overridePath,
  });
}

/**
 * Cancels/removes the workspace override for a session.
 * @param sessionId The session ID to configure.
 * @returns A promise that resolves when the override is cancelled.
 */
export async function cancelWorkspaceOverride(
  sessionId: string,
): Promise<void> {
  return safeInvoke<void>('cancel_workspace_override', { sessionId });
}

/**
 * Returns the absolute filesystem path of the session's workspace directory.
 * Used to construct file:// URLs for binary file indexing in the Content Store.
 * @param sessionId The session ID to query.
 * @returns A promise that resolves to the absolute workspace directory path string.
 */
export async function getWorkspaceDir(sessionId: string): Promise<string> {
  return safeInvoke<string>('get_workspace_dir', { sessionId });
}

/**
 * Lists all file paths within the session workspace up to `maxDepth` levels deep.
 * Returns relative paths (forward-slash separated) sorted alphabetically.
 * @param sessionId The session ID to query.
 * @param maxDepth Maximum directory traversal depth.
 * @returns A promise that resolves to an array of relative path strings.
 */
export async function listWorkspaceFilePaths(
  sessionId: string,
  maxDepth: number,
): Promise<string[]> {
  return safeInvoke<string[]>('list_workspace_file_paths', {
    sessionId,
    maxDepth,
  });
}

/**
 * Lists all file paths within an arbitrary workspace directory up to `maxDepth` levels deep.
 * Returns relative paths (forward-slash separated) sorted alphabetically.
 * @param workspacePath The absolute workspace directory path to query.
 * @param maxDepth Maximum directory traversal depth.
 * @returns A promise that resolves to an array of relative path strings.
 */
export async function listWorkspaceFilePathsForPath(
  workspacePath: string,
  maxDepth: number,
): Promise<string[]> {
  return safeInvoke<string[]>('list_workspace_file_paths_for_path', {
    workspacePath,
    maxDepth,
  });
}
