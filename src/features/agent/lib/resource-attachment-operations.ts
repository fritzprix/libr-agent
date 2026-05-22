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

const TEXT_EXTENSIONS =
  /\.(txt|md|markdown|json|jsonc|json5|yaml|yml|toml|js|jsx|ts|tsx|mjs|cjs|py|rb|rs|go|java|c|cpp|h|hpp|css|scss|less|html|htm|svg|sh|bash|zsh|fish|ps1|sql|graphql|csv|log|xml|proto)$/i;

const SUPPORTED_EXTENSIONS =
  /\.(txt|md|markdown|json|jsonc|json5|yaml|yml|toml|js|jsx|ts|tsx|mjs|cjs|py|rb|rs|go|java|c|cpp|h|hpp|css|scss|less|html|htm|svg|sh|bash|zsh|fish|ps1|sql|graphql|csv|log|xml|proto|pdf|docx|xlsx)$/i;

function isTextFile(filename: string, mimeType: string): boolean {
  return (
    /^text\/|\/(json|xml|javascript|typescript)/.test(mimeType) ||
    TEXT_EXTENSIONS.test(filename)
  );
}

interface AddAgentAttachmentArgs {
  sessionId: string;
  url: string;
  mimeType: string;
  filename?: string;
  file?: File;
  inlineAudio?: boolean;
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
  file: File | undefined,
  sessionId: string,
  filename: string,
  mimeType: string,
): Promise<ResolvedAttachmentSource> {
  if (!file) {
    logger.warn('resolveAttachmentSourceFromFile received undefined file', {
      filename,
    });
    return {
      fileUrl: '',
      actualMimeType: mimeType || getMimeTypeFromFilename(filename),
      fileSize: 0,
    };
  }
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
  let sourceBlob = file ?? fetchedBlob;

  if (!sourceBlob && fileUrl) {
    try {
      logger.info('Fetching fallback source blob from fileUrl', { fileUrl });
      const response = await fetch(fileUrl);
      if (response.ok) {
        sourceBlob = await response.blob();
      }
    } catch (fetchError) {
      logger.warn(
        'Failed to fetch blob from fileUrl for inline data extraction',
        {
          fileUrl,
          fetchError,
        },
      );
    }
  }

  if (sourceBlob) {
    try {
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

export function toWorkspaceOnlyAttachment(
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
  let content: string | undefined;
  let lineCount = 0;

  if (file && isTextFile(filename, mimeType)) {
    try {
      content = await file.text();
      lineCount = content.split('\n').length;
    } catch (error) {
      logger.warn('Failed to read text content for indexing', {
        filename,
        error,
      });
    }
  }

  const result = await saveAgentFile(sessionId, filename, {
    content,
    fileUrl: content ? undefined : fileUrl,
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
    lineCount: result.lineCount ?? lineCount,
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
  inlineAudio = true,
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
    (source.actualMimeType.startsWith('audio/') && inlineAudio !== false);

  try {
    if (isInlineType) {
      try {
        return await toInlineAttachment(
          sessionId,
          resolvedFilename,
          source.actualMimeType,
          source.fileUrl,
          source.fileSize,
          file,
          source.fetchedBlob,
        );
      } catch (error) {
        logger.warn(
          'Failed to build inline media attachment, falling back to workspace-only',
          {
            filename: resolvedFilename,
            error: error instanceof Error ? error.message : String(error),
          },
        );
        return toWorkspaceOnlyAttachment(
          sessionId,
          resolvedFilename,
          source.actualMimeType,
          source.fileSize,
          source.workspacePath,
        );
      }
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

    try {
      return await commitAttachmentToStore(
        sessionId,
        resolvedFilename,
        source.actualMimeType,
        source.fileUrl,
        source.fileSize,
        source.workspacePath,
        file,
      );
    } catch (error) {
      logger.warn(
        'Failed to commit attachment to search index store, falling back to workspace-only',
        {
          filename: resolvedFilename,
          error: error instanceof Error ? error.message : String(error),
        },
      );
      return toWorkspaceOnlyAttachment(
        sessionId,
        resolvedFilename,
        source.actualMimeType,
        source.fileSize,
        source.workspacePath,
      );
    }
  } finally {
    if (source.fileUrl.startsWith('blob:')) {
      URL.revokeObjectURL(source.fileUrl);
    }
  }
}
