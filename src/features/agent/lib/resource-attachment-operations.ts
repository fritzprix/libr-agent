import { getLogger } from '@/lib/logger';
import { syncFileToWorkspace } from '@/lib/workspace-sync-service';
import { getWorkspaceDir } from '@/lib/backend/workspace';
import { workspacePathToFileUrl } from '@/lib/file-url';
import { getMimeTypeFromFilename } from '@/lib/mime-utils';
import { saveAgentFile } from '@/features/agent/api/agent-backend';
import type { AttachmentReference } from '@/models/chat';
import {
  convertToBlobUrl,
  extractFilenameFromUrl,
} from './resource-attachment-utils';

const logger = getLogger('resourceAttachmentOperations');

const SUPPORTED_EXTENSIONS = /\.(txt|md|json|pdf|docx|xlsx)$/i;

interface AddAgentAttachmentArgs {
  sessionId: string;
  url: string;
  mimeType: string;
  filename?: string;
  file?: File;
}

interface ResolvedAttachmentSource {
  fileUrl: string;
  actualMimeType: string;
  fileSize: number;
  workspacePath?: string;
  fetchedBlob?: Blob;
}

function resolveMimeType(
  requestedMimeType: string,
  blobType: string | undefined,
  filename: string,
): string {
  if (requestedMimeType && requestedMimeType !== 'application/octet-stream') {
    return requestedMimeType;
  }

  return blobType || requestedMimeType || getMimeTypeFromFilename(filename);
}

async function resolveAttachmentSourceFromFile(
  file: File,
  sessionId: string,
  filename: string,
  mimeType: string,
): Promise<ResolvedAttachmentSource> {
  try {
    const workspacePath = await syncFileToWorkspace(file, sessionId);
    const workspaceDir = await getWorkspaceDir(sessionId);

    return {
      fileUrl: workspacePathToFileUrl(workspaceDir, workspacePath),
      actualMimeType:
        file.type || mimeType || getMimeTypeFromFilename(filename),
      fileSize: file.size,
      workspacePath,
    };
  } catch (error) {
    logger.warn('Workspace sync failed, falling back to blob URL', {
      filename,
      error: error instanceof Error ? error.message : String(error),
    });

    return {
      fileUrl: URL.createObjectURL(file),
      actualMimeType:
        file.type || mimeType || getMimeTypeFromFilename(filename),
      fileSize: file.size,
    };
  }
}

async function resolveAttachmentSourceFromUrl(
  url: string,
  sessionId: string,
  filename: string,
  mimeType: string,
): Promise<ResolvedAttachmentSource> {
  try {
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`Failed to fetch ${url}`);
    }

    const fetchedBlob = await response.blob();
    const resolvedMimeType = resolveMimeType(
      mimeType,
      fetchedBlob.type,
      filename,
    );
    const downloadedFile = new File([fetchedBlob], filename, {
      type: resolvedMimeType,
    });
    const workspacePath = await syncFileToWorkspace(downloadedFile, sessionId);
    const workspaceDir = await getWorkspaceDir(sessionId);

    return {
      fileUrl: workspacePathToFileUrl(workspaceDir, workspacePath),
      actualMimeType: resolvedMimeType,
      fileSize: fetchedBlob.size,
      workspacePath,
      fetchedBlob,
    };
  } catch (error) {
    logger.warn('URL download failed, falling back to blob URL', {
      url,
      filename,
      error: error instanceof Error ? error.message : String(error),
    });

    const blobResult = await convertToBlobUrl(url);
    return {
      fileUrl: blobResult.blobUrl,
      actualMimeType:
        mimeType || blobResult.type || getMimeTypeFromFilename(filename),
      fileSize: blobResult.size || 0,
    };
  }
}

async function toInlineAttachment(
  sessionId: string,
  filename: string,
  mimeType: string,
  fileUrl: string,
  fileSize: number,
  file?: File,
  fetchedBlob?: Blob,
): Promise<AttachmentReference> {
  logger.info('File is image/audio — storing stable inline media reference', {
    filename,
    mimeType,
    hasStableUri: !fileUrl.startsWith('blob:'),
  });

  const inlineType = mimeType.startsWith('image/')
    ? ('image' as const)
    : ('audio' as const);

  let fallbackBase64Data: string | undefined;
  if (fileUrl.startsWith('blob:')) {
    try {
      const sourceBlob: Blob =
        file ??
        fetchedBlob ??
        (() => {
          throw new Error('No data source available for inline content');
        })();
      const buffer = await sourceBlob.arrayBuffer();
      const bytes = new Uint8Array(buffer);
      let binary = '';
      for (const byte of bytes) {
        binary += String.fromCharCode(byte);
      }
      fallbackBase64Data = btoa(binary);
    } catch (error) {
      logger.error('Failed to read fallback inline media as base64', {
        filename,
        error,
      });
      throw new Error(
        `Failed to read file "${filename}" for inline attachment.`,
      );
    }
  }

  return {
    sessionId,
    status: 'inline',
    filename,
    mimeType,
    size: fileSize,
    lineCount: 0,
    preview: filename,
    uploadedAt: new Date().toISOString(),
    inlineContent: {
      type: inlineType,
      data: fallbackBase64Data,
      uri: fileUrl,
      mimeType,
    },
  };
}

function toWorkspaceOnlyAttachment(
  sessionId: string,
  filename: string,
  mimeType: string,
  fileSize: number,
  workspacePath?: string,
): AttachmentReference {
  logger.info(
    'File type not supported by Attachments, saving to workspace only',
    {
      filename,
    },
  );

  return {
    sessionId,
    status: 'workspace-only',
    filename,
    mimeType,
    size: fileSize,
    lineCount: 0,
    preview: filename,
    uploadedAt: new Date().toISOString(),
    workspacePath,
  };
}

async function commitAttachmentToStore(
  sessionId: string,
  filename: string,
  mimeType: string,
  fileUrl: string,
  fileSize: number,
  workspacePath?: string,
  file?: File,
): Promise<AttachmentReference> {
  const result = await saveAgentFile(sessionId, filename, {
    content: undefined,
    fileUrl,
    metadata: {
      mimeType,
      size: fileSize,
      uploadedAt: new Date().toISOString(),
      filename,
    },
  });

  let resolvedWorkspacePath = workspacePath;
  if (!resolvedWorkspacePath && file) {
    try {
      resolvedWorkspacePath = await syncFileToWorkspace(file, sessionId);
    } catch (error) {
      logger.warn(
        'Workspace sync failed (retry), continuing with content-store only',
        { error },
      );
    }
  }

  logger.debug('[AgentResourceAttachmentContext] saveAgentFile result:', {
    result,
  });

  return {
    sessionId: result.sessionId,
    contentId: result.contentId,
    status: 'committed',
    filename: result.filename ?? filename,
    mimeType: result.mimeType,
    size: Number(result.size ?? fileSize ?? 0),
    lineCount: result.lineCount,
    preview: result.preview,
    uploadedAt: result.uploadedAt ?? new Date().toISOString(),
    chunkCount: result.chunkCount,
    lastAccessedAt: new Date().toISOString(),
    workspacePath: resolvedWorkspacePath,
  };
}

export async function addAgentAttachment({
  sessionId,
  url,
  mimeType,
  filename,
  file,
}: AddAgentAttachmentArgs): Promise<AttachmentReference> {
  const resolvedFilename = filename || extractFilenameFromUrl(url);
  const source = file
    ? await resolveAttachmentSourceFromFile(
        file,
        sessionId,
        resolvedFilename,
        mimeType,
      )
    : await resolveAttachmentSourceFromUrl(
        url,
        sessionId,
        resolvedFilename,
        mimeType,
      );

  if (
    source.actualMimeType === 'application/octet-stream' ||
    source.actualMimeType === ''
  ) {
    logger.warn(
      'Could not resolve MIME type from file.type, mimeType param, or extension',
      { filename: resolvedFilename },
    );
  }

  const isInlineType =
    source.actualMimeType.startsWith('image/') ||
    source.actualMimeType.startsWith('audio/');

  try {
    if (isInlineType) {
      return toInlineAttachment(
        sessionId,
        resolvedFilename,
        source.actualMimeType,
        source.fileUrl,
        source.fileSize,
        file,
        source.fetchedBlob,
      );
    }

    if (!SUPPORTED_EXTENSIONS.test(resolvedFilename)) {
      return toWorkspaceOnlyAttachment(
        sessionId,
        resolvedFilename,
        source.actualMimeType,
        source.fileSize,
        source.workspacePath,
      );
    }

    return await commitAttachmentToStore(
      sessionId,
      resolvedFilename,
      source.actualMimeType,
      source.fileUrl,
      source.fileSize,
      source.workspacePath,
      file,
    );
  } finally {
    if (source.fileUrl.startsWith('blob:')) {
      URL.revokeObjectURL(source.fileUrl);
    }
  }
}
