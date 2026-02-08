import { safeInvoke } from './core';

// ========================================
// File System Operations
// ========================================

/**
 * Reads the content of a file that was dropped onto the application window.
 * @param filePath The path of the dropped file.
 * @returns A promise that resolves to an array of numbers representing the file's byte content.
 */
export async function readDroppedFile(filePath: string): Promise<number[]> {
  return safeInvoke<number[]>('read_dropped_file', { filePath });
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
