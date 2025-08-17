import TerminalHeader from '@/components/TerminalHeader';
import {
  Button,
  CompactModelPicker,
  FileAttachment,
  Input,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui';
import { Send, X } from 'lucide-react';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext, ChatProvider } from '@/context/ChatContext';
import { BuiltInToolsSystemPrompt } from '@/features/prompts/BuiltInToolsSystemPrompt';
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import { useWebMCPServer } from '@/context/WebMCPContext';
import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { getLogger } from '@/lib/logger';
import { AttachmentReference, Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useRustBackend } from '@/hooks/use-rust-backend';
import ToolsModal from '../tools/ToolsModal';
import MessageBubble from './MessageBubble';
import { TimeLocationSystemPrompt } from '../prompts/TimeLocationSystemPrompt';
// import { useWebMCPServer } from '@/context/WebMCPContext';
// import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';

const logger = getLogger('Chat');

interface ChatProps {
  children?: React.ReactNode;
}

// Main Chat container component
function Chat({ children }: ChatProps) {
  const [showToolsDetail, setShowToolsDetail] = useState(false);
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }

  return (
    <ChatProvider>
      <BuiltInToolsSystemPrompt />
      <TimeLocationSystemPrompt />
      {/* <JailbreakSystemPrompt /> */}
      <div className="h-full w-full font-mono flex flex-col rounded-lg overflow-hidden shadow-2xl">
        {children}
        <ToolsModal
          isOpen={showToolsDetail}
          onClose={() => setShowToolsDetail(false)}
        />
      </div>
    </ChatProvider>
  );
}

// Session Files Popover component
function SessionFilesPopover({ storeId }: { storeId: string }) {
  const {
    sessionFiles,
    isSessionFilesLoading,
    sessionFilesError,
    refreshSessionFiles,
  } = useResourceAttachment();

  const [isOpen, setIsOpen] = useState(false);
  const [selectedFile, setSelectedFile] = useState<AttachmentReference | null>(
    null,
  );
  const [fileContent, setFileContent] = useState<string>('');
  const [isLoadingContent, setIsLoadingContent] = useState(false);
  const { server } = useWebMCPServer<ContentStoreServer>('content-store');

  const handleFileClick = useCallback(
    async (file: AttachmentReference) => {
      setSelectedFile(file);
      setIsLoadingContent(true);

      try {
        // 먼저 preview 사용
        let content = file.preview || '';

        // preview가 없거나 짧으면 전체 내용 가져오기
        if (!content || content.length < 100) {
          if (server) {
            logger.debug('Loading full file content', {
              storeId: file.storeId,
              contentId: file.contentId,
              filename: file.filename,
            });

            const result = await server.readContent({
              storeId: file.storeId,
              contentId: file.contentId,
              lineRange: { fromLine: 1 }, // 전체 파일 읽기
            });

            content = result?.content || 'File content not available';
          } else {
            content = 'Content store server not available';
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
    [server],
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
            <p className="text-xs text-gray-400">Store ID: {storeId}</p>
          </div>

          {isSessionFilesLoading ? (
            <div className="p-4 text-center text-xs text-gray-400">
              파일 목록 로딩 중...
            </div>
          ) : sessionFilesError ? (
            <div className="p-4 text-center text-xs text-red-400">
              {sessionFilesError}
              <button
                onClick={refreshSessionFiles}
                className="block mt-2 text-blue-400 hover:underline text-xs"
              >
                다시 시도
              </button>
            </div>
          ) : sessionFiles.length === 0 ? (
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
                          <span>{formatDate(file.uploadedAt)}</span>
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

      {/* 파일 내용 상세 보기 Dialog */}
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
                <span>생성: {formatDate(selectedFile.uploadedAt)}</span>
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

// Chat Header component
function ChatHeader({
  children,
  assistantName,
}: {
  children?: React.ReactNode;
  assistantName?: string;
}) {
  const { current: currentSession } = useSessionContext();

  return (
    <TerminalHeader>
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-blue-400">
              [{assistantName}]
            </span>
          )}
        </div>

        {/* 세션에 storeId가 있을 때만 파일함 버튼 표시 */}
        {currentSession?.storeId && (
          <SessionFilesPopover storeId={currentSession.storeId} />
        )}
      </div>
    </TerminalHeader>
  );
}

// Chat Messages component
function ChatMessages() {
  const { messages, isLoading } = useChatContext();
  const { getCurrentSession, current: currentSession } = useSessionContext();
  const { getById } = useAssistantContext();

  const messagesEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, messagesEndRef]);

  // 각 메시지의 assistantId로 이름을 찾아서 전달
  const getAssistantNameForMessage = useCallback(
    (m: Message) => {
      if (m.role === 'assistant' && 'assistantId' in m && m.assistantId) {
        const assistant = getById(m.assistantId);
        return assistant?.name || '';
      }
      // fallback: current session의 첫 assistant
      const currentSession = getCurrentSession();
      if (m.role === 'assistant' && currentSession?.assistants?.length) {
        return currentSession.assistants[0].name;
      }
      return '';
    },
    [getById, getCurrentSession],
  );

  return (
    <div className="flex-1 flex flex-col min-h-0">
      <div className="flex-1 p-4 overflow-y-auto flex flex-col gap-6 terminal-scrollbar">
        {messages.map((m) => (
          <MessageBubble
            key={m.id}
            message={m}
            currentAssistantName={getAssistantNameForMessage(m)}
          />
        ))}
        {isLoading && (
          <div className="flex justify-start">
            <div className="rounded px-3 py-2">
              <div className="text-xs mb-1">
                Agent ({currentSession?.assistants[0]?.name})
              </div>
              <div className="text-sm">thinking...</div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}

// Chat Status Bar component - now manages its own tools state
function ChatStatusBar({
  children,
  onShowTools,
}: {
  children?: React.ReactNode;
  onShowTools?: () => void;
}) {
  const { availableTools, isLoading, error } = useMCPServer();

  // 로딩 스피너 컴포넌트
  const LoadingSpinner = () => (
    <svg
      className="animate-spin h-3 w-3 text-yellow-400"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );

  // 도구 상태를 계산
  const getToolsDisplayText = () => {
    if (isLoading) return 'Loading tools...';
    if (error) return 'Tools error';
    return `${availableTools.length} available`;
  };

  const getToolsColor = () => {
    if (isLoading) return 'text-yellow-400';
    if (error) return 'text-red-400';
    return availableTools.length > 0 ? 'text-green-400' : 'text-gray-500';
  };

  const getToolsIcon = () => {
    if (isLoading) return <LoadingSpinner />;
    if (error) return '⚠️';
    return '🔧';
  };

  return (
    <div className="px-4 py-2 border-t flex items-center justify-between">
      <div>
        <CompactModelPicker />
        {children}
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs">Tools:</span>
        <button
          onClick={onShowTools}
          className={`text-xs transition-colors flex items-center gap-1 ${getToolsColor()}`}
          disabled={isLoading}
          title={error || undefined}
        >
          {getToolsIcon()} {getToolsDisplayText()}
        </button>
      </div>
    </div>
  );
}

// Chat Attached Files component - now uses ResourceAttachmentContext
function ChatAttachedFiles() {
  const { files: attachedFiles, removeFile } = useResourceAttachment();

  const removeAttachedFile = React.useCallback(
    (file: AttachmentReference) => {
      removeFile(file);
    },
    [removeFile],
  );

  if (attachedFiles.length === 0) return null;

  return (
    <div className="px-4 py-2 border-t">
      <div className="text-xs mb-2">📎 Attached Files:</div>
      <div className="flex flex-wrap gap-2">
        {attachedFiles.map((file, index) => (
          <div
            key={index}
            className="flex items-center px-2 py-1 rounded border border-gray-700"
          >
            <span className="text-xs truncate max-w-[150px]">
              {file.filename}
            </span>
            <Button
              type="button"
              onClick={() => removeAttachedFile(file)}
              className="ml-2 text-xs"
            >
              ✕
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
// Chat Input component - now uses ResourceAttachmentContext
function ChatInput({ children }: { children?: React.ReactNode }) {
  const [input, setInput] = useState<string>('');
  const [dragState, setDragState] = useState<'none' | 'valid' | 'invalid'>(
    'none',
  );
  const dropTimeoutRef = useRef<ReturnType<typeof setTimeout>>();

  // File validation function
  const validateFiles = useCallback((filePaths: string[]): boolean => {
    const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
    return filePaths.every((path) => {
      const filename = path.split('/').pop() || path.split('\\').pop() || '';
      return supportedExtensions.test(filename);
    });
  }, []);

  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { submit, isLoading, cancel } = useChatContext();
  const {
    files: attachedFiles,
    addFile,
    removeFile,
    clearFiles,
    isLoading: isAttachmentLoading,
  } = useResourceAttachment();
  const rustBackend = useRustBackend();

  // Handle drag and drop events from Tauri
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupDragDrop = async () => {
      try {
        const webview = getCurrentWebview();
        unlisten = await webview.onDragDropEvent((event) => {
          logger.debug('Drag drop event:', event);

          if (event.payload.type === 'enter') {
            const isValid = validateFiles(event.payload.paths);
            setDragState(isValid ? 'valid' : 'invalid');
          } else if (event.payload.type === 'drop') {
            setDragState('none');
            handleFileDrop(event.payload.paths);
          } else if (event.payload.type === 'leave') {
            setDragState('none');
          }
        });
      } catch (error) {
        logger.warn('Failed to setup drag and drop listener:', error);
      }
    };

    setupDragDrop();

    return () => {
      if (unlisten) {
        unlisten();
      }
      if (dropTimeoutRef.current) {
        clearTimeout(dropTimeoutRef.current);
      }
    };
  }, []);

  // Process files with actual file operations
  const processFileDrop = useCallback(
    async (filePaths: string[]) => {
      if (!currentSession) {
        alert('Cannot attach file: session not available.');
        return;
      }

      logger.info('Files dropped:', filePaths);

      for (const filePath of filePaths) {
        let blobUrl = '';
        try {
          // Extract filename from path
          const filename =
            filePath.split('/').pop() ||
            filePath.split('\\').pop() ||
            'unknown';

          // Check file extension
          const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
          if (!supportedExtensions.test(filename)) {
            alert(`File "${filename}" format is not supported.`);
            continue;
          }

          logger.debug(`Processing dropped file`, {
            filePath,
            filename,
            sessionId: currentSession?.id,
          });

          // Read file content using type-safe Rust backend
          const fileData = await rustBackend.readFile(filePath);

          // Convert number array to Uint8Array and create a blob
          const uint8Array = new Uint8Array(fileData);
          const blob = new Blob([uint8Array]);
          blobUrl = URL.createObjectURL(blob);

          // Determine MIME type based on file extension
          const getMimeType = (filename: string): string => {
            const ext = filename.toLowerCase().split('.').pop();
            switch (ext) {
              case 'txt':
                return 'text/plain';
              case 'md':
                return 'text/markdown';
              case 'json':
                return 'application/json';
              case 'pdf':
                return 'application/pdf';
              case 'docx':
                return 'application/vnd.openxmlformats-officedocument.wordprocessingml.document';
              case 'xlsx':
                return 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet';
              default:
                return 'application/octet-stream';
            }
          };

          const mimeType = getMimeType(filename);

          // Use the blob URL with the ResourceAttachmentContext
          await addFile(blobUrl, mimeType, filename);

          logger.info(`Dropped file processed successfully`, {
            filename,
            filePath,
            mimeType,
          });
        } catch (error) {
          logger.error(`Error processing dropped file ${filePath}:`, {
            filePath,
            sessionId: currentSession?.id,
            error:
              error instanceof Error
                ? {
                    message: error.message,
                    stack: error.stack,
                    name: error.name,
                  }
                : error,
            errorString: String(error),
          });
          alert(
            `Error processing file "${filePath}": ${error instanceof Error ? error.message : String(error)}`,
          );
        } finally {
          // Clean up blob URL to prevent memory leaks
          if (blobUrl) {
            URL.revokeObjectURL(blobUrl);
          }
        }
      }
    },
    [currentSession, addFile],
  );

  // Handle dropped files with debounce to prevent duplicate events
  const handleFileDrop = useCallback(
    (filePaths: string[]) => {
      // Clear existing timeout
      if (dropTimeoutRef.current) {
        clearTimeout(dropTimeoutRef.current);
      }

      // Short delay to prevent duplicate events
      dropTimeoutRef.current = setTimeout(() => {
        processFileDrop(filePaths);
      }, 10);
    },
    [processFileDrop],
  );

  const handleAgentInputChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setInput(e.target.value);
    },
    [setInput],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();

      // If loading, cancel the request
      if (isLoading) {
        cancel();
        return;
      }

      // Submit if there's text OR if files are attached.
      if (!input.trim() && attachedFiles.length === 0) return;
      if (!currentAssistant || !currentSession) return;

      const userMessage: Message = {
        id: createId(),
        content: input.trim(),
        role: 'user',
        sessionId: currentSession.id,
        // Include the references of successfully attached files.
        attachments: attachedFiles,
      };

      setInput('');
      // Clear the attached files from the UI after submission.
      clearFiles();

      try {
        await submit([userMessage]);
      } catch (err) {
        logger.error('Error submitting message:', err);
      }
    },
    [
      submit,
      input,
      attachedFiles,
      currentAssistant,
      currentSession,
      clearFiles,
      isLoading,
      cancel,
    ],
  );

  // File attachment handler using ResourceAttachmentContext
  const handleFileAttachment = React.useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || !currentSession) {
        alert('Cannot attach file: session not available.');
        return;
      }

      // Process each selected file.
      for (const file of files) {
        // You can expand this list based on what your parsers support.
        const supportedExtensions = /\.(txt|md|json|pdf|docx|xlsx)$/i;
        if (!supportedExtensions.test(file.name)) {
          alert(`File "${file.name}" format is not supported.`);
          continue;
        }

        // Example file size limit: 50MB
        if (file.size > 50 * 1024 * 1024) {
          alert(`File "${file.name}" is too large. Maximum size is 50MB.`);
          continue;
        }

        let fileUrl = '';
        try {
          logger.debug(`Starting file processing`, {
            filename: file.name,
            fileSize: file.size,
            fileType: file.type,
            sessionId: currentSession?.id,
          });

          // 1. Create a temporary blob URL for the file.
          fileUrl = URL.createObjectURL(file);

          // 2. Use ResourceAttachmentContext to add file
          await addFile(fileUrl, file.type, file.name);

          logger.info(`File processed successfully`, {
            filename: file.name,
            fileSize: file.size,
          });
        } catch (error) {
          logger.error(`Error processing file ${file.name}:`, {
            filename: file.name,
            fileSize: file.size,
            fileType: file.type,
            sessionId: currentSession?.id,
            error:
              error instanceof Error
                ? {
                    message: error.message,
                    stack: error.stack,
                    name: error.name,
                  }
                : error,
            errorString: String(error),
          });
          alert(
            `Error processing file "${file.name}": ${error instanceof Error ? error.message : String(error)}`,
          );
        } finally {
          // 3. IMPORTANT: Revoke the blob URL to prevent memory leaks.
          if (fileUrl) {
            URL.revokeObjectURL(fileUrl);
          }
        }
      }

      // Clear the file input so the user can select the same file again.
      e.target.value = '';
    },
    [currentSession, addFile],
  );

  // Remove file handler using ResourceAttachmentContext
  const removeAttachedFile = React.useCallback(
    (filename: string) => {
      const fileToRemove = attachedFiles.find(
        (f: AttachmentReference) => f.filename === filename,
      );
      if (fileToRemove) {
        removeFile(fileToRemove);
      }
    },
    [attachedFiles, removeFile],
  );

  return (
    <form
      onSubmit={handleSubmit}
      className={`px-4 py-4 border-t flex items-center gap-2 transition-colors ${
        dragState === 'valid'
          ? 'bg-green-500/10 border-green-500'
          : dragState === 'invalid'
            ? 'bg-destructive/10 border-destructive'
            : ''
      }`}
    >
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <Input
          value={input}
          onChange={handleAgentInputChange}
          placeholder={
            dragState !== 'none'
              ? dragState === 'valid'
                ? 'Drop supported files here...'
                : 'Unsupported file type!'
              : isLoading || isAttachmentLoading
                ? 'Agent busy...'
                : 'Query agent or drop files...'
          }
          disabled={isLoading || isAttachmentLoading}
          className={`flex-1 min-w-0 transition-colors ${
            dragState === 'valid'
              ? 'border-green-500 bg-green-500/10'
              : dragState === 'invalid'
                ? 'border-destructive bg-destructive/10'
                : ''
          }`}
          autoComplete="off"
          spellCheck="false"
        />

        <FileAttachment
          // Convert AttachmentReference[] to the expected format
          files={attachedFiles.map((file: AttachmentReference) => ({
            name: file.filename,
            content: file.preview || '',
          }))}
          onRemove={(index: number) => {
            const file = attachedFiles[index];
            if (file) {
              removeAttachedFile(file.filename);
            }
          }}
          onAdd={handleFileAttachment}
          compact={true}
        />
        {children}
      </div>

      <Button
        type="submit"
        // Disable button if there's nothing to send (only when not loading).
        disabled={
          isAttachmentLoading ||
          (!isLoading && !input.trim() && attachedFiles.length === 0)
        }
        variant="ghost"
        size="icon"
        title={isLoading ? 'Cancel request' : 'Send message'}
      >
        {isLoading ? <X className="h-4 w-4" /> : <Send className="h-4 w-4" />}
      </Button>
    </form>
  );
}

// Chat Bottom section (combines status bar, attached files, and input)
function ChatBottom({ children }: { children?: React.ReactNode }) {
  const [showToolsDetail, setShowToolsDetail] = useState(false);

  return (
    <div className="flex-shrink-0">
      <ChatStatusBar onShowTools={() => setShowToolsDetail(true)} />
      <ChatAttachedFiles />
      <ChatInput />
      <ToolsModal
        isOpen={showToolsDetail}
        onClose={() => setShowToolsDetail(false)}
      />
      {children}
    </div>
  );
}

// Attach subcomponents as static properties
Chat.Header = ChatHeader;
Chat.Messages = ChatMessages;
Chat.StatusBar = ChatStatusBar;
Chat.AttachedFiles = ChatAttachedFiles;
Chat.Input = ChatInput;
Chat.Bottom = ChatBottom;

export default Chat;
