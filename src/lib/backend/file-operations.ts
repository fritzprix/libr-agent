import { safeInvoke } from './core';

// ========================================
// File System Operations
// ========================================

/**
 * Checks if a dropped path is a file or a directory.
 *
 * For **directories**: the path is consumed from the allowlist immediately.
 * For **files**: the path is only verified to exist in the allowlist; it is NOT
 * consumed here — consumption happens later when {@link readDroppedFile} is called.
 *
 * Callers must not assume a file path is unusable after this check.
 *
 * @param path The path of the dropped item.
 * @returns A promise that resolves to 'file' or 'directory'.
 */
export async function checkDroppedPathType(
  path: string,
): Promise<'file' | 'directory'> {
  return safeInvoke<'file' | 'directory'>('check_dropped_path_type', { path });
}

/**
 * Reads the content of a file that was dropped onto the application window.
 * @param filePath The path of the dropped file.
 * @returns A promise that resolves to an array of numbers representing the file's byte content.
 */
export async function readDroppedFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_dropped_file', { filePath });
}

/**
 * Registers file paths that were delivered by an OS file-drop event.
 * Backend will only allow `read_dropped_file` for paths from this allowlist.
 */
export async function registerDroppedFiles(paths: string[]): Promise<void> {
  return safeInvoke<void>('register_dropped_files', { paths });
}

/**
 * Writes content to a file in the filesystem.
 * @param filePath The path to the file to write to.
 * @param content An array of numbers representing the byte content to write.
 * @returns A promise that resolves when the write operation is complete.
 */
export async function writeFile(
  filePath: string,
  content: number[],
): Promise<void> {
  return safeInvoke<void>('write_file', { filePath, content });
}
