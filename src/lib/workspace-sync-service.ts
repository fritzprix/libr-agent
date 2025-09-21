import { workspaceWriteFile } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';

const logger = getLogger('WorkspaceSync');

/** The maximum file size for the content store (50MB). */
export const MAX_CONTENT_STORE_SIZE = 50 * 1024 * 1024;
/** The maximum file size for the workspace (10MB). */
export const MAX_WORKSPACE_SIZE = 10 * 1024 * 1024;
/**
 * The effective maximum file size, which is the more restrictive of the
 * content store and workspace limits.
 */
export const EFFECTIVE_MAX_SIZE = Math.min(
  MAX_CONTENT_STORE_SIZE,
  MAX_WORKSPACE_SIZE,
);

/**
 * Synchronizes a file to the workspace storage system.
 * This involves validating the file size, converting the file to a byte array,
 * generating a safe workspace path, and invoking the Rust backend to write the file.
 *
 * @param file The `File` object to synchronize.
 * @returns A promise that resolves to the relative path of the file in the workspace.
 * @throws An error if the file size exceeds the limit or if the backend operation fails.
 */
export async function syncFileToWorkspace(file: File): Promise<string> {
  logger.info('Starting workspace sync', {
    filename: file.name,
    fileSize: file.size,
  });

  try {
    // Validate file size before processing
    if (file.size > EFFECTIVE_MAX_SIZE) {
      throw new Error(
        `File size ${file.size} bytes exceeds maximum allowed size ${EFFECTIVE_MAX_SIZE} bytes`,
      );
    }

    // Generate workspace path
    const workspacePath = generateWorkspacePath(file.name);

    // Convert File object to number array for Rust backend
    const arrayBuffer = await file.arrayBuffer();
    const uint8Array = new Uint8Array(arrayBuffer);
    const numberArray = Array.from(uint8Array);

    logger.info('Converting file to workspace format', {
      filename: file.name,
      workspacePath,
      originalSize: file.size,
      convertedSize: numberArray.length,
    });

    // Save file to workspace via Rust backend (session-aware)
    await workspaceWriteFile(workspacePath, numberArray);

    logger.info('File synced to workspace successfully', {
      filename: file.name,
      workspacePath,
      size: file.size,
    });

    return workspacePath;
  } catch (error) {
    logger.error('Failed to sync file to workspace', {
      filename: file.name,
      error: error instanceof Error ? error.message : String(error),
    });
    throw error;
  }
}

/**
 * Generates a unique and safe relative path for a file in the workspace.
 * It prepends a timestamp to the sanitized filename to avoid collisions.
 *
 * @param filename The original filename.
 * @returns A relative path string suitable for use with the backend's file manager.
 */
export function generateWorkspacePath(filename: string): string {
  const timestamp = Date.now();
  const sanitizedFilename = sanitizeFilename(filename);
  return `attachments/${timestamp}_${sanitizedFilename}`;
}

/**
 * Sanitizes a filename to make it safe for use in a filesystem path.
 * It replaces unsafe characters and whitespace with underscores and truncates the length.
 *
 * @param filename The original filename.
 * @returns The sanitized filename.
 * @internal
 */
function sanitizeFilename(filename: string): string {
  // Remove or replace unsafe characters
  return filename
    .replace(/[<>:"/\\|?*]/g, '_') // Replace unsafe characters with underscore
    .replace(/\s+/g, '_') // Replace spaces with underscore
    .replace(/_{2,}/g, '_') // Replace multiple underscores with single
    .trim()
    .slice(0, 200); // Limit length to prevent path issues
}

/**
 * Validates if a file's size is within the effective maximum limit.
 *
 * @param file The `File` object to validate.
 * @returns True if the file size is acceptable, false otherwise.
 */
export function validateFileSize(file: File): boolean {
  return file.size <= EFFECTIVE_MAX_SIZE;
}

/**
 * Gets the effective maximum file size in megabytes (MB) for display purposes.
 *
 * @returns The maximum file size in MB.
 */
export function getMaxFileSizeMB(): number {
  return EFFECTIVE_MAX_SIZE / (1024 * 1024);
}

/**
 * Creates a human-readable error message for a file that exceeds the size limit.
 *
 * @param filename The name of the file that is too large.
 * @param actualSize The actual size of the file in bytes.
 * @returns A formatted error message string.
 */
export function createFileSizeErrorMessage(
  filename: string,
  actualSize: number,
): string {
  const maxSizeMB = getMaxFileSizeMB();
  const actualSizeMB = (actualSize / (1024 * 1024)).toFixed(1);
  return `File "${filename}" is too large (${actualSizeMB}MB). Maximum size is ${maxSizeMB}MB.`;
}
