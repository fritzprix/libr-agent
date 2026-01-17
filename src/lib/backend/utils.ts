import { safeInvoke } from './core';
import type {
  ServiceContext,
  ServiceContextOptions,
} from '@/features/tools/types';

// ========================================
// Log Management
// ========================================

/**
 * Gets the directory where application logs are stored.
 * @returns A promise that resolves to the absolute path of the log directory.
 */
export async function getAppLogsDir(): Promise<string> {
  return safeInvoke<string>('get_app_logs_dir');
}

/**
 * Creates a backup of the current log file.
 * @returns A promise that resolves to the path of the newly created backup file.
 */
export async function backupCurrentLog(): Promise<string> {
  return safeInvoke<string>('backup_current_log');
}

/**
 * Clears the content of the current log file.
 * @returns A promise that resolves when the log file has been cleared.
 */
export async function clearCurrentLog(): Promise<void> {
  return safeInvoke<void>('clear_current_log');
}

/**
 * Lists all log files in the application's log directory.
 * @returns A promise that resolves to an array of log file names.
 */
export async function listLogFiles(): Promise<string[]> {
  return safeInvoke<string[]>('list_log_files');
}

// ========================================
// External URL Handling
// ========================================

/**
 * Opens a URL in the user's default external browser.
 * @param url The URL to open.
 * @returns A promise that resolves when the URL has been opened.
 */
export async function openExternalUrl(url: string): Promise<void> {
  return safeInvoke<void>('open_external_url', { url });
}

// ========================================
// File Download Operations
// ========================================

/**
 * Initiates a download of a file from the workspace.
 * @param filePath The path of the file within the workspace to download.
 * @param sessionId The ID of the session to download from.
 * @returns A promise that resolves to a string indicating the download status or path.
 */
export async function downloadWorkspaceFile(
  filePath: string,
  sessionId: string,
): Promise<string> {
  return safeInvoke<string>('download_workspace_file', {
    filePath,
    sessionId,
  });
}

/**
 * Exports a selection of files as a zip archive and initiates a download.
 * @param files An array of file paths to include in the zip archive.
 * @param packageName The name for the zip package.
 * @param sessionId The ID of the session to export from.
 * @returns A promise that resolves to a string indicating the download status or path.
 */
export async function exportAndDownloadZip(
  files: string[],
  packageName: string,
  sessionId: string,
): Promise<string> {
  return safeInvoke<string>('export_and_download_zip', {
    files,
    packageName,
    sessionId,
  });
}

// ========================================
// Service Context
// ========================================

/**
 * Retrieves the service context for a given server.
 * @param serverId The ID of the server.
 * @param options Optional context options for the service.
 * @returns A promise that resolves to the service context.
 */
export async function getServiceContext(
  serverId: string,
  options?: ServiceContextOptions,
): Promise<ServiceContext<unknown>> {
  return safeInvoke<ServiceContext<unknown>>('get_service_context', {
    serverId,
    options,
  });
}

// ========================================
// Miscellaneous
// ========================================

/**
 * A simple utility function to test the backend connection.
 * @param name A name to include in the greeting.
 * @returns A promise that resolves to a greeting string from the backend.
 */
export async function greet(name: string): Promise<string> {
  return safeInvoke<string>('greet', { name });
}
