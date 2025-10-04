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
 * The implementation follows clear, named steps for maintainability:
 * 1) normalizeUnicode → 2) replaceUnsafeChars → 3) collapseWhitespace → 4) limitLength
 * 5) splitBaseAndExt → 6) sanitizeBase/Extension → 7) recombine → 8) finalCleanup
 *
 * Behavior preserved from previous implementation (200 char limit, lowercase ext,
 * collapse underscores, drop invalid extension chars, ensure non-empty base → "file").
 *
 * Note: Exported for unit testing. Marked as internal API.
 *
 * @param filename The original filename.
 * @returns The sanitized filename.
 * @internal
 */
export function sanitizeFilename(filename: string): string {
  // 1) Unicode normalization (NFKC) for canonical representation
  const normalizeUnicode = (name: string) => name.normalize('NFKC');

  // 2) Replace filesystem-unsafe characters and whitespace with underscores
  const replaceUnsafeChars = (name: string) =>
    name.replace(/[<>:"/\\|?*]/g, '_');

  const collapseWhitespace = (name: string) => name.replace(/\s+/g, '_');

  // 3) Collapse multiple underscores and trim spaces (spaces are already converted)
  const collapseUnderscores = (name: string) => name.replace(/_{2,}/g, '_');

  // 4) Enforce global length cap early to bound subsequent operations
  const limitLength = (name: string, max = 200) => name.slice(0, max);

  // 5) Split into base and extension by last dot (dot at index 0 means no ext)
  const splitBaseAndExt = (name: string): { base: string; ext: string } => {
    const idx = name.lastIndexOf('.');
    if (idx > 0) {
      return { base: name.slice(0, idx), ext: name.slice(idx + 1) };
    }
    return { base: name, ext: '' };
  };

  // 6a) Sanitize base: collapse dots to underscores, trim leading/trailing underscores
  const sanitizeBase = (base: string): string => {
    const cleaned = base.replace(/\.+/g, '_').replace(/^_+|_+$/g, '');
    return cleaned || 'file';
  };

  // 6b) Sanitize extension: remove dots, non-alnum, lowercase
  const sanitizeExtension = (ext: string): string =>
    ext.replace(/\.+/g, '').replace(/[^A-Za-z0-9]/g, '').toLowerCase();

  // 7) Recombine
  const recombine = (base: string, ext: string): string =>
    ext.length > 0 ? `${base}.${ext}` : base;

  // 8) Final cleanup: collapse repeated dots, remove pathologically repeated sequences
  const finalCleanup = (name: string): string => {
    let safe = name
      .replace(/\.+/g, '.')
      .replace(/\.{2}/g, '_')
      .replace(/_{2,}/g, '_')
      .replace(/^_+|_+$/g, '');
    if (!safe) safe = 'file';
    return limitLength(safe);
  };

  // Pipeline
  const step1 = normalizeUnicode(filename);
  const step2 = replaceUnsafeChars(step1);
  const step3 = collapseWhitespace(step2);
  const step4 = collapseUnderscores(step3).trim();
  const step5 = limitLength(step4);
  const { base, ext } = splitBaseAndExt(step5);
  const cleanBase = sanitizeBase(base);
  const cleanExt = sanitizeExtension(ext);
  const combined = recombine(cleanBase, cleanExt);
  return finalCleanup(combined);
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
