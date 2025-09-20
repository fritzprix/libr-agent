import { workspaceWriteFile } from '@/lib/rust-backend-client';
import { getLogger } from '@/lib/logger';

const logger = getLogger('WorkspaceSync');

/**
 * File size limits (in bytes)
 * Using the more restrictive limit to ensure compatibility with both systems
 */
export const MAX_CONTENT_STORE_SIZE = 50 * 1024 * 1024; // 50MB
export const MAX_WORKSPACE_SIZE = 10 * 1024 * 1024; // 10MB
export const EFFECTIVE_MAX_SIZE = Math.min(
  MAX_CONTENT_STORE_SIZE,
  MAX_WORKSPACE_SIZE,
); // 10MB

/**
 * Synchronizes a file to the workspace storage system
 * @param file - File object to synchronize
 * @param sessionId - Current session ID for workspace context (sets Rust backend session)
 * @returns Promise resolving to the relative workspace file path
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
 * Generates a unique workspace path for file storage
 * @param filename - Original filename
 * @returns Workspace path string (relative path for Tauri SecureFileManager)
 */
export function generateWorkspacePath(filename: string): string {
  const timestamp = Date.now();
  const sanitizedFilename = sanitizeFilename(filename);
  return `attachments/${timestamp}_${sanitizedFilename}`;
}

/**
 * Sanitizes filename for safe filesystem usage
 * @param filename - Original filename
 * @returns Sanitized filename
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
 * Validates file size against unified limits
 * @param file - File to validate
 * @returns boolean indicating if file size is acceptable
 */
export function validateFileSize(file: File): boolean {
  return file.size <= EFFECTIVE_MAX_SIZE;
}

/**
 * Gets the effective maximum file size in MB for display purposes
 * @returns Maximum file size in MB
 */
export function getMaxFileSizeMB(): number {
  return EFFECTIVE_MAX_SIZE / (1024 * 1024);
}

/**
 * Creates a human-readable file size error message
 * @param filename - Name of the file that exceeded limits
 * @param actualSize - Actual file size in bytes
 * @returns Error message string
 */
export function createFileSizeErrorMessage(
  filename: string,
  actualSize: number,
): string {
  const maxSizeMB = getMaxFileSizeMB();
  const actualSizeMB = (actualSize / (1024 * 1024)).toFixed(1);
  return `File "${filename}" is too large (${actualSizeMB}MB). Maximum size is ${maxSizeMB}MB.`;
}
