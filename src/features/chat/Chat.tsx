import TerminalHeader from '@/components/TerminalHeader';
import {
  Button,
  CompactModelPicker,
  FileAttachment,
  Input,
} from '@/components/ui';
import { useAssistantContext } from '@/context/AssistantContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatContext, ChatProvider } from '@/context/ChatContext';
import { useResourceAttachment } from '@/context/ResourceAttachmentContext';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { getLogger } from '@/lib/logger';
import { AttachmentReference, Message } from '@/models/chat';
import { createId } from '@paralleldrive/cuid2';
import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import ToolsModal from '../tools/ToolsModal';
import MessageBubble from './MessageBubble';
import { ToolCaller } from './ToolCaller';
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
      <div className="h-full w-full font-mono flex flex-col rounded-lg overflow-hidden shadow-2xl">
        {children}
        <ToolCaller />
        <ToolsModal
          isOpen={showToolsDetail}
          onClose={() => setShowToolsDetail(false)}
        />
      </div>
    </ChatProvider>
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
  return (
    <TerminalHeader>
      {children}
      {assistantName && (
        <span className="ml-2 text-xs text-blue-400">[{assistantName}]</span>
      )}
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
  // Move tools logic here since it's only used in this component
  const { availableTools: mcpTools } = useMCPServer();
  const availableTools = useMemo(
    () => [...mcpTools],
    [mcpTools],
  );

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
          className="text-xs transition-colors flex items-center gap-1"
        >
          🔧 {availableTools.length} available
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

  const { current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  const { submit, isLoading } = useChatContext();
  const {
    files: attachedFiles,
    addFile,
    removeFile,
    clearFiles,
    isLoading: isAttachmentLoading,
  } = useResourceAttachment();

  const handleAgentInputChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setInput(e.target.value);
    },
    [setInput],
  );

  const handleSubmit = useCallback(
    async (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();
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
      className="px-4 py-4 border-t flex items-center gap-2"
    >
      <span className="font-bold flex-shrink-0">$</span>
      <div className="flex-1 flex items-center gap-2 min-w-0">
        <Input
          value={input}
          onChange={handleAgentInputChange}
          placeholder={
            isLoading || isAttachmentLoading
              ? 'Agent busy...'
              : 'Query agent...'
          }
          disabled={isLoading || isAttachmentLoading}
          className="flex-1 min-w-0"
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
        // Disable button if there's nothing to send.
        disabled={
          isLoading ||
          isAttachmentLoading ||
          (!input.trim() && attachedFiles.length === 0)
        }
        variant="ghost"
        size="sm"
        className="px-1"
      >
        ⏎
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
