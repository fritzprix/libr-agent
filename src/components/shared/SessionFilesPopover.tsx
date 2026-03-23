import { useCallback, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Folder, FolderOpen, FileText } from 'lucide-react';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { getLogger } from '@/lib/logger';
import { AttachmentReference } from '@/models/chat';
import { agentCallBuiltinTool } from '@/features/agent/api/agent-backend';

const logger = getLogger('SessionFilesPopover');

interface SessionFilesPopoverProps {
  sessionId: string;
}

export function SessionFilesPopover({ sessionId }: SessionFilesPopoverProps) {
  const { t } = useTranslation('common');
  const { sessionFiles } = useAgentResourceAttachment();
  const [isOpen, setIsOpen] = useState(false);
  const [selectedFile, setSelectedFile] = useState<AttachmentReference | null>(
    null,
  );
  const [fileContent, setFileContent] = useState<string>('');
  const [isLoadingContent, setIsLoadingContent] = useState(false);

  // Filter files for current session and reset state when session changes
  const currentSessionFiles = useMemo(() => {
    return sessionFiles.filter((file) => file.sessionId === sessionId);
  }, [sessionFiles, sessionId]);

  const handleFileClick = useCallback(async (file: AttachmentReference) => {
    setSelectedFile(file);
    setIsLoadingContent(true);

    try {
      let content = file.preview || '';

      if (!content || content.length < 100) {
        logger.debug('Loading full file content via builtin tool', {
          sessionId: file.sessionId,
          contentId: file.contentId,
          filename: file.filename,
        });

        const result = await agentCallBuiltinTool(
          file.sessionId,
          'attachments__read',
          {
            sessionId: file.sessionId,
            contentId: file.contentId,
            lineRange: { fromLine: 1 },
          },
        );

        // Parse the result
        // The result from agentCallBuiltinTool is strict MCPResult
        if (result && typeof result === 'object' && 'content' in result) {
          // Check for structuredContent first (Agent V2 pattern)
          // But built-in tools often return TextContent or EmbeddedResource
          const textContent = Array.isArray(result.content)
            ? result.content.find((c) => c.type === 'text')?.text
            : undefined;

          if (textContent) {
            content = textContent;
          } else if ('structuredContent' in result) {
            // Some tools might still use this custom field?
            // Checking types.rs/MCPResult, it has content: Vec<MCPContent>
            // So structuredContent might be deprecated or non-standard.
            // However, let's keep the logic aligned with what the legacy tool did if possible.
            // Legacy tool seemed to handle a custom 'structuredContent' field in result.
            // Let's assume the Rust tool returns standard MCP content now.
            content = 'File content format not supported for display.';
          } else {
            content = 'File content not availble.';
          }
        } else {
          content = 'File content not available';
        }

        // Refined logic based on expected Rust output for read
        // The Rust `read_content` returns a TextContent block with the file data.
        // Or if it was structured, it might return a JSON string in text.
        if (
          result &&
          Array.isArray(result.content) &&
          result.content.length > 0
        ) {
          const item = result.content[0];
          if (item.type === 'text') {
            content = item.text;
          }
        }
      }

      setFileContent(content);
      logger.debug('Successfully loaded file content', {
        filename: file.filename,
        contentLength: content.length,
      });
    } catch (error) {
      logger.error('Failed to load file content:', {
        filename: file.filename,
        error: error instanceof Error ? error.message : String(error),
      });
      setFileContent(t('sessionFiles.readError', 'Error reading file.'));
    } finally {
      setIsLoadingContent(false);
    }
  }, []);

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  };

  const formatDate = (dateString: string) => {
    return new Date(dateString).toLocaleDateString('ko-KR', {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  return (
    <>
      <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
        <DropdownMenuTrigger asChild>
          <button
            className="text-xs hover:text-primary transition-colors flex items-center gap-1 focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-sm"
            title={t('sessionFiles.viewFiles', 'View session files')}
          >
            <Folder className="w-3 h-3" />
            <span>{currentSessionFiles.length}</span>
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-80 p-0" side="bottom" align="end">
          <div className="border-b px-3 py-2">
            <h4 className="text-sm font-medium">
              {t('sessionFiles.fileList', 'Session file list')}
            </h4>
            <p className="text-xs text-muted-foreground">
              {t('sessionFiles.sessionId', 'Session ID:')} {sessionId}
            </p>
          </div>

          {currentSessionFiles.length === 0 ? (
            <div className="p-4 text-center text-xs text-muted-foreground">
              {t('sessionFiles.noFiles', 'No saved files.')}
            </div>
          ) : (
            <div className="max-h-64 overflow-y-auto">
              {currentSessionFiles.map((file) => (
                <DropdownMenuItem
                  key={
                    file.contentId ??
                    file.pendingId ??
                    file.workspacePath ??
                    `${file.filename}-${file.uploadedAt}`
                  }
                  className="px-3 py-2 cursor-pointer border-b last:border-b-0 block"
                  onClick={() => handleFileClick(file)}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      <div className="text-xs font-medium truncate">
                        {file.filename}
                      </div>
                      <div className="text-xs text-muted-foreground mt-1">
                        {file.mimeType && (
                          <span className="mr-2">{file.mimeType}</span>
                        )}
                        {file.size && (
                          <span className="mr-2">
                            {formatFileSize(file.size)}
                          </span>
                        )}
                        {file.uploadedAt && (
                          <span className="mr-2">
                            {formatDate(file.uploadedAt)}
                          </span>
                        )}
                        {file.workspacePath && (
                          <span className="text-success flex items-center gap-1">
                            <FolderOpen className="w-3 h-3" />
                            {t('sessionFiles.workspaceValue', 'Workspace')}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <FileText className="w-4 h-4" />
                    </div>
                  </div>
                  {file.preview && (
                    <div className="text-xs text-muted-foreground mt-1 truncate">
                      {file.preview.slice(0, 50)}...
                    </div>
                  )}
                </DropdownMenuItem>
              ))}
            </div>
          )}
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog
        open={!!selectedFile}
        onOpenChange={(open) => !open && setSelectedFile(null)}
      >
        <DialogContent className="max-w-4xl max-h-[80%] flex flex-col">
          <DialogHeader>
            <DialogTitle className="text-sm">
              {selectedFile?.filename}
            </DialogTitle>
            <div className="text-xs text-muted-foreground">
              {selectedFile?.mimeType && (
                <span className="mr-4">
                  {t('sessionFiles.type', 'Type:')} {selectedFile.mimeType}
                </span>
              )}
              {selectedFile?.size && (
                <span className="mr-4">
                  {t('sessionFiles.size', 'Size:')}{' '}
                  {formatFileSize(selectedFile.size)}
                </span>
              )}
              {selectedFile?.uploadedAt && (
                <span className="mr-4">
                  {t('sessionFiles.created', 'Created:')}{' '}
                  {formatDate(selectedFile.uploadedAt)}
                </span>
              )}
              {selectedFile?.workspacePath && (
                <span className="text-success">
                  {t('sessionFiles.workspace', 'Workspace:')}{' '}
                  {selectedFile.workspacePath}
                </span>
              )}
            </div>
          </DialogHeader>

          <div className="flex-1 min-h-0 mt-4">
            {isLoadingContent ? (
              <div className="flex items-center justify-center h-32">
                <div className="text-sm text-muted-foreground">
                  {t('common.loading', 'Loading...')}
                </div>
              </div>
            ) : (
              <div className="h-full overflow-auto border rounded p-3 bg-muted">
                <pre className="text-xs whitespace-pre-wrap font-mono">
                  {fileContent}
                </pre>
              </div>
            )}
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}
