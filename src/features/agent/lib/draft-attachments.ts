import { getLogger } from '@/lib/logger';
import { workspaceWriteFile, getWorkspaceDir } from '@/lib/backend';
import { generateWorkspacePath } from '@/lib/workspace-sync-service';
import { workspacePathToFileUrl } from '@/lib/file-url';
import { saveAgentFile } from '@/features/agent/api/agent-backend';
import type { AttachmentReference } from '@/models/chat';

const logger = getLogger('draftAttachmentHelpers');

export const TEXT_EXTENSIONS_DRAFT =
  /\.(txt|md|markdown|json|jsonc|json5|yaml|yml|toml|js|jsx|ts|tsx|mjs|cjs|py|rb|rs|go|java|c|cpp|h|hpp|css|scss|less|html|htm|svg|sh|bash|zsh|fish|ps1|sql|graphql|csv|log|xml|proto)$/i;

export const BINARY_INDEXABLE_EXTENSIONS_DRAFT = /\.(pdf|docx|xlsx)$/i;

interface PrepareDraftAttachmentsArgs {
  files: File[];
  sessionId: string;
  now: Date;
  getMimeType: (filename: string) => string;
  onAttachmentError: (file: File) => void;
}

export async function prepareDraftAttachments({
  files,
  sessionId,
  now,
  getMimeType,
  onAttachmentError,
}: PrepareDraftAttachmentsArgs): Promise<AttachmentReference[]> {
  const attachments: AttachmentReference[] = [];
  const workspaceDir = await getWorkspaceDir(sessionId);

  for (const file of files) {
    try {
      const workspacePath = generateWorkspacePath(file.name);
      const arrayBuffer = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(arrayBuffer));
      await workspaceWriteFile(workspacePath, bytes, sessionId);

      let lineCount = 0;
      const isText =
        /^text\/|\/(json|xml|javascript|typescript)/.test(file.type) ||
        TEXT_EXTENSIONS_DRAFT.test(file.name);
      if (isText) {
        try {
          const text = await file.text();
          lineCount = text.split('\n').length;
        } catch {
          lineCount = 0;
        }
      }

      const actualMimeType =
        file.type && file.type !== 'application/octet-stream'
          ? file.type
          : getMimeType(file.name);
      const isInlineType =
        actualMimeType.startsWith('image/') ||
        actualMimeType.startsWith('audio/');

      if (isInlineType) {
        const inlineType = actualMimeType.startsWith('image/')
          ? ('image' as const)
          : ('audio' as const);
        const fileUrl = workspacePathToFileUrl(workspaceDir, workspacePath);
        attachments.push({
          sessionId,
          filename: file.name,
          mimeType: actualMimeType,
          size: file.size,
          lineCount: 0,
          preview: file.name,
          uploadedAt: now.toISOString(),
          status: 'inline',
          workspacePath,
          inlineContent: {
            type: inlineType,
            uri: fileUrl,
            mimeType: actualMimeType,
          },
        });
      } else {
        attachments.push({
          sessionId,
          filename: file.name,
          mimeType: actualMimeType,
          size: file.size,
          lineCount,
          preview: file.name,
          uploadedAt: now.toISOString(),
          status: 'workspace-only',
          workspacePath,
        });
      }
    } catch (error) {
      logger.error('Failed to write pre-session attachment', error);
      onAttachmentError(file);
    }
  }

  let workspaceDirCache: string | null = null;
  const getWorkspaceDirCached = async (): Promise<string> => {
    if (workspaceDirCache === null) {
      workspaceDirCache = await getWorkspaceDir(sessionId);
    }
    return workspaceDirCache;
  };

  for (let index = 0; index < files.length; index++) {
    const file = files[index];
    const attachment = attachments[index];
    if (!attachment) {
      continue;
    }

    const isTextFile =
      TEXT_EXTENSIONS_DRAFT.test(file.name) || /^text\//.test(file.type);
    const isBinaryIndexable = BINARY_INDEXABLE_EXTENSIONS_DRAFT.test(file.name);

    if (isTextFile) {
      try {
        const content = await file.text();
        const result = await saveAgentFile(sessionId, file.name, {
          content,
          metadata: {
            mimeType: file.type || 'text/plain',
            size: file.size,
            uploadedAt: now.toISOString(),
            filename: file.name,
          },
        });

        if (result && typeof result === 'object' && 'contentId' in result) {
          attachments[index] = {
            ...attachment,
            status: 'committed',
            contentId: result.contentId,
            lineCount: result.lineCount ?? attachment.lineCount,
          };
        }
      } catch (error) {
        logger.warn(
          'Failed to commit file to Attachments store, keeping workspace-only',
          { filename: file.name, error },
        );
      }
    } else if (isBinaryIndexable) {
      try {
        const cachedWorkspaceDir = await getWorkspaceDirCached();
        const workspacePath = attachment.workspacePath;
        if (!workspacePath) {
          logger.warn('Binary file missing workspacePath, skipping index', {
            filename: file.name,
          });
          continue;
        }
        const normalizedDir = cachedWorkspaceDir.replace(/\\/g, '/');
        const normalizedRelative = workspacePath.replace(/\\/g, '/');
        const fileUrl = `file:///${normalizedDir.replace(/^\//, '')}/${normalizedRelative}`;
        const result = await saveAgentFile(sessionId, file.name, {
          fileUrl,
          metadata: {
            mimeType: file.type || 'application/octet-stream',
            size: file.size,
            uploadedAt: now.toISOString(),
            filename: file.name,
          },
        });

        if (result && typeof result === 'object' && 'contentId' in result) {
          attachments[index] = {
            ...attachment,
            status: 'committed',
            contentId: result.contentId,
            lineCount: result.lineCount ?? attachment.lineCount,
          };
        }
      } catch (error) {
        logger.warn(
          'Failed to commit binary file to Attachments store, keeping workspace-only',
          { filename: file.name, error },
        );
      }
    }
  }

  return attachments;
}
