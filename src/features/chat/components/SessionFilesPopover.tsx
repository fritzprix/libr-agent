import { useCallback, useState } from 'react';
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
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import { getLogger } from '@/lib/logger';
import { AttachmentReference } from '@/models/chat';
import { useServiceContext } from '@/features/tools/useServiceContext';
import { useBuiltInTool } from '@/features/tools';

const logger = getLogger('SessionFilesPopover');

interface SessionFilesPopoverProps {
  sessionId: string;
}

export function SessionFilesPopover({ sessionId }: SessionFilesPopoverProps) {
  const { sessionFiles } = useResourceAttachment();
  // Ensure contentstore service context is loaded (for future structured state)
  useServiceContext('contentstore');
  const { executeTool } = useBuiltInTool();
  const [isOpen, setIsOpen] = useState(false);
  const [selectedFile, setSelectedFile] = useState<AttachmentReference | null>(
    null,
  );
  const [fileContent, setFileContent] = useState<string>('');
  const [isLoadingContent, setIsLoadingContent] = useState(false);

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

          const result = await executeTool({
            id: `read_content_${Date.now()}`,
            type: 'function',
            function: {
              name: 'builtin_contentstore__readContent',
              arguments: JSON.stringify({
                sessionId: file.sessionId,
                contentId: file.contentId,
                lineRange: { fromLine: 1 },
              }),
            },
          });

          // Parse the result
          if (result.result?.structuredContent) {
            // If it's structured content, extract the content field
            const contentData = result.result.structuredContent as {
              content: string;
              lineRange: [number, number];
            };
            content = contentData.content || 'File content not available';
          } else if (
            result.result?.content &&
            result.result.content[0] &&
            'text' in result.result.content[0]
          ) {
            // Fallback to text content
            content = (result.result.content[0] as { text: string }).text;
          } else {
            content = 'File content not available';
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
        setFileContent('파일을 읽는 중 오류가 발생했습니다.');
      } finally {
        setIsLoadingContent(false);
      }
    },
    [executeTool],
  );

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
            className="text-xs hover:text-blue-400 transition-colors flex items-center gap-1"
            title="세션 파일 보기"
          >
            📁 {sessionFiles.length}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-80 p-0" side="bottom" align="end">
          <div className="border-b px-3 py-2">
            <h4 className="text-sm font-medium">세션 파일 목록</h4>
            <p className="text-xs text-gray-400">Session ID: {sessionId}</p>
          </div>

          {sessionFiles.length === 0 ? (
            <div className="p-4 text-center text-xs text-gray-400">
              저장된 파일이 없습니다.
            </div>
          ) : (
            <div className="max-h-64 overflow-y-auto">
              {sessionFiles.map((file, index) => (
                <DropdownMenuItem
                  key={index}
                  className="px-3 py-2 cursor-pointer border-b last:border-b-0 block"
                  onClick={() => handleFileClick(file)}
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      <div className="text-xs font-medium truncate">
                        {file.filename}
                      </div>
                      <div className="text-xs text-gray-400 mt-1">
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
                          <span className="text-green-400">📁 Workspace</span>
                        )}
                      </div>
                    </div>
                    <div className="text-xs text-gray-400">📄</div>
                  </div>
                  {file.preview && (
                    <div className="text-xs text-gray-400 mt-1 truncate">
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
        <DialogContent className="max-w-4xl max-h-[80vh] flex flex-col">
          <DialogHeader>
            <DialogTitle className="text-sm">
              {selectedFile?.filename}
            </DialogTitle>
            <div className="text-xs text-gray-400">
              {selectedFile?.mimeType && (
                <span className="mr-4">타입: {selectedFile.mimeType}</span>
              )}
              {selectedFile?.size && (
                <span className="mr-4">
                  크기: {formatFileSize(selectedFile.size)}
                </span>
              )}
              {selectedFile?.uploadedAt && (
                <span className="mr-4">
                  생성: {formatDate(selectedFile.uploadedAt)}
                </span>
              )}
              {selectedFile?.workspacePath && (
                <span className="text-green-400">
                  워크스페이스: {selectedFile.workspacePath}
                </span>
              )}
            </div>
          </DialogHeader>

          <div className="flex-1 min-h-0 mt-4">
            {isLoadingContent ? (
              <div className="flex items-center justify-center h-32">
                <div className="text-sm text-gray-400">로딩 중...</div>
              </div>
            ) : (
              <div className="h-full overflow-auto border rounded p-3 bg-gray-900/30">
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
