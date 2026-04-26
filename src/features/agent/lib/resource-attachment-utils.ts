import { getLogger } from '@/lib/logger';
import type { AttachmentReference } from '@/models/chat';

const logger = getLogger('resourceAttachmentUtils');

export interface PendingFileInput {
  file?: File;
  url: string;
  filename?: string;
  mimeType: string;
  originalPath?: string;
  status?: AttachmentReference['status'];
  blobCleanup?: () => void;
}

export function extractFilenameFromUrl(url: string): string {
  try {
    const urlObj = new URL(url);
    const pathname = urlObj.pathname;
    return pathname.split('/').pop() || 'unknown_file';
  } catch {
    return `file_${Date.now()}`;
  }
}

export async function convertToBlobUrl(url: string): Promise<{
  blobUrl: string;
  cleanup: () => void;
  size: number;
  type: string;
}> {
  try {
    if (url.startsWith('blob:')) {
      return {
        blobUrl: url,
        cleanup: () => {},
        size: 0,
        type: '',
      };
    }

    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(
        `Failed to fetch ${url}: ${response.status} ${response.statusText}`,
      );
    }

    const blob = await response.blob();
    const blobUrl = URL.createObjectURL(blob);

    return {
      blobUrl,
      cleanup: () => URL.revokeObjectURL(blobUrl),
      size: blob.size,
      type: blob.type,
    };
  } catch (error) {
    logger.error('Failed to convert URL to blob', { url, error });
    throw new Error(
      `Failed to process URL "${url}": ${error instanceof Error ? error.message : String(error)}`,
    );
  }
}

export function createPendingAttachmentReferences(
  files: PendingFileInput[],
  sessionId: string,
): AttachmentReference[] {
  const nowIso = new Date().toISOString();

  return files.map((file) => {
    const resolvedFilename = file.filename || extractFilenameFromUrl(file.url);

    return {
      sessionId,
      pendingId: `pending_${Date.now()}_${Math.random().toString(36).slice(2, 11)}`,
      status: file.status || ('pending' as const),
      filename: resolvedFilename,
      mimeType: file.mimeType,
      size: file.file?.size || 0,
      lineCount: 0,
      preview: resolvedFilename,
      uploadedAt: nowIso,
      chunkCount: 0,
      lastAccessedAt: nowIso,
      originalUrl: file.url,
      originalPath: file.originalPath,
      file: file.file,
      blobCleanup: file.blobCleanup,
    };
  });
}

export function cleanupPendingAttachmentBlobs(
  files: AttachmentReference[],
): void {
  files.forEach((file) => {
    if (!file.blobCleanup) {
      return;
    }

    try {
      file.blobCleanup();
    } catch (error) {
      logger.warn('Blob cleanup failed', error);
    }
  });
}
