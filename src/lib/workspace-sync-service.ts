import { workspaceWriteFile } from '@/lib/backend/workspace';
import { settingsService } from '@/lib/services/settings-service';

/**
 * Synchronizes a file to the workspace storage system.
 * This involves validating the file size, converting the file to a byte array,
 * generating a safe workspace path, and invoking the Rust backend to write the file.
 *
 * @param file The `File` object to synchronize.
 * @param sessionId The optional session ID to specify which session's workspace to sync to.
 * @returns A promise that resolves to the relative path of the file in the workspace.
 * @throws An error if the file size exceeds the limit or if the backend operation fails.
 */
export async function syncFileToWorkspace(
  file: File,
  sessionId?: string,
): Promise<string> {
  const settings = await settingsService.getSettings();
  const maxFileUploadBytes = settings.system.maxFileUploadSizeMB * 1024 * 1024;
  const workspaceCapBytes = settings.system.workspaceCapacityMB * 1024 * 1024;
  // Effective max size for a single file is the smaller of the two (safe default logic)
  const effectiveLimit = Math.min(maxFileUploadBytes, workspaceCapBytes);

  // Validate file size before processing
  if (file.size > effectiveLimit) {
    throw new Error(
      `File size ${file.size} bytes exceeds maximum allowed size ${effectiveLimit} bytes`,
    );
  }

  // Generate workspace path
  const workspacePath = generateWorkspacePath(file.name);

  // Convert File object to number array for Rust backend
  const arrayBuffer = await file.arrayBuffer();
  const uint8Array = new Uint8Array(arrayBuffer);
  const numberArray = Array.from(uint8Array);

  // Save file to workspace via Rust backend (session-aware)
  await workspaceWriteFile(workspacePath, numberArray, sessionId);

  return workspacePath;
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
 * Normalizes the text to Unicode NFC/NFKC format to handle composed/precomposed character differences.
 * @internal
 */
// Sanitize helpers exported for readability and testing
export function normalizeUnicode(name: string): string {
  return name.normalize('NFKC');
}

export function replaceUnsafeChars(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, '_');
}

export function collapseWhitespace(name: string): string {
  return name.replace(/\s+/g, '_');
}

export function collapseUnderscores(name: string): string {
  return name.replace(/_{2,}/g, '_');
}

export function limitLength(name: string, max = 200): string {
  return name.slice(0, max);
}

export function splitBaseAndExt(name: string): { base: string; ext: string } {
  const idx = name.lastIndexOf('.');
  if (idx > 0) {
    return { base: name.slice(0, idx), ext: name.slice(idx + 1) };
  }
  return { base: name, ext: '' };
}

export function sanitizeBase(base: string): string {
  const cleaned = base.replace(/\.+/g, '_').replace(/^_+|_+$/g, '');
  return cleaned || 'file';
}

export function sanitizeExtension(ext: string): string {
  return ext
    .replace(/\.+/g, '')
    .replace(/[^A-Za-z0-9]/g, '')
    .toLowerCase();
}

export function recombineFilename(base: string, ext: string): string {
  return ext.length > 0 ? `${base}.${ext}` : base;
}

export function finalCleanup(name: string): string {
  let safe = name
    .replace(/\.+/g, '.')
    .replace(/\.{2}/g, '_')
    .replace(/_{2,}/g, '_')
    .replace(/^_+|_+$/g, '');
  if (!safe) safe = 'file';
  return limitLength(safe);
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
  const step1 = normalizeUnicode(filename);
  const step2 = replaceUnsafeChars(step1);
  const step3 = collapseWhitespace(step2);
  const step4 = collapseUnderscores(step3).trim();
  const step5 = limitLength(step4);
  const { base, ext } = splitBaseAndExt(step5);
  const cleanBase = sanitizeBase(base);
  const cleanExt = sanitizeExtension(ext);
  const combined = recombineFilename(cleanBase, cleanExt);
  return finalCleanup(combined);
}

/**
 * Validates if a file's size is within the effective maximum limit.
 *
 * @param file The `File` object to validate.
 * @param maxSizeBytes The maximum allowed size in bytes.
 * @returns True if the file size is acceptable, false otherwise.
 */
export function validateFileSize(file: File, maxSizeBytes: number): boolean {
  return file.size <= maxSizeBytes;
}

/**
 * Creates a human-readable error message for a file that exceeds the size limit.
 *
 * @param filename The name of the file that is too large.
 * @param actualSize The actual size of the file in bytes.
 * @param maxSizeBytes The maximum allowed size in bytes.
 * @returns A formatted error message string.
 */
export function createFileSizeErrorMessage(
  filename: string,
  actualSize: number,
  maxSizeBytes: number,
): string {
  const maxSizeMB = maxSizeBytes / (1024 * 1024);
  const actualSizeMB = (actualSize / (1024 * 1024)).toFixed(1);
  return `File "${filename}" is too large (${actualSizeMB}MB). Maximum size is ${maxSizeMB.toFixed(1)}MB.`;
}
