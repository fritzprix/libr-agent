import { useCallback, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Paperclip, FolderOpen, FileText } from 'lucide-react';
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui';
import { useAgentResourceAttachment } from '@/features/agent/hooks/useAgentResourceAttachment';
import { getLogger } from '@/lib/logger';
import { AttachmentReference } from '@/models/chat';
import { agentCallBuiltinTool } from '@/features/agent/api/agent-backend';
import { getDateTimeFormatter } from '@/lib/date-utils';

const logger = getLogger('SessionFilesPopover');
const SESSION_FILE_DATE_FORMATTER = getDateTimeFormatter('ko-KR', {
  month: 'short',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
});

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

  const handleFileClick = useCallback(
    async (file: AttachmentReference) => {
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
            'attachments__readAttachment',
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
              content = 'File content not available.';
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
    },
    [t],
  );

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes}B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  };

  const formatDate = (dateString: string) => {
    return SESSION_FILE_DATE_FORMATTER.format(new Date(dateString));
  };

  const viewFilesLabel = t('sessionFiles.viewFiles', 'View session files');

  return (
    <>
      <DropdownMenu open={isOpen} onOpenChange={setIsOpen}>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                aria-label={viewFilesLabel}
                className="h-6 gap-1 px-2"
              >
                <Paperclip className="h-4 w-4" />
                <span className="text-xs">{currentSessionFiles.length}</span>
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{viewFilesLabel}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent
          className="w-80 border bg-popover p-0 text-popover-foreground shadow-md"
          side="bottom"
          align="end"
        >
          <div className="border-b bg-muted/40 px-3 py-2">
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
                  className="block cursor-pointer border-b px-3 py-2 last:border-b-0"
                  onClick={() => handleFileClick(file)}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-xs font-medium">
                        {file.filename}
                      </div>
                      <div className="mt-1 text-xs text-muted-foreground">
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
                          <span className="flex items-center gap-1 text-success">
                            <FolderOpen className="h-3 w-3" />
                            {t('sessionFiles.workspaceValue', 'Workspace')}
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="text-xs text-muted-foreground">
                      <FileText className="h-4 w-4" />
                    </div>
                  </div>
                  {file.preview && (
                    <div className="mt-1 truncate text-xs text-muted-foreground">
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
        <DialogContent className="flex max-h-[80%] max-w-4xl flex-col">
          <DialogHeader>
            <DialogTitle className="text-sm">
              {selectedFile?.filename}
            </DialogTitle>
            <DialogDescription className="sr-only">
              {[
                selectedFile?.mimeType
                  ? `${t('sessionFiles.type', 'Type:')} ${selectedFile.mimeType}`
                  : null,
                selectedFile?.size
                  ? `${t('sessionFiles.size', 'Size:')} ${formatFileSize(selectedFile.size)}`
                  : null,
                selectedFile?.uploadedAt
                  ? `${t('sessionFiles.created', 'Created:')} ${formatDate(selectedFile.uploadedAt)}`
                  : null,
                selectedFile?.workspacePath
                  ? `${t('sessionFiles.workspace', 'Workspace:')} ${selectedFile.workspacePath}`
                  : null,
              ]
                .filter((value): value is string => Boolean(value))
                .join(' • ')}
            </DialogDescription>
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

          <div className="mt-4 min-h-0 flex-1">
            {isLoadingContent ? (
              <div className="flex h-32 items-center justify-center">
                <div className="text-sm text-muted-foreground">
                  {t('common.loading', 'Loading...')}
                </div>
              </div>
            ) : (
              <div className="h-full overflow-auto rounded border bg-muted p-3">
                <pre className="whitespace-pre-wrap font-mono text-xs">
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
